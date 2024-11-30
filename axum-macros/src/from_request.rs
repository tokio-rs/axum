use self::attr::FromRequestContainerAttrs;
use crate::{
    attr_parsing::{parse_attrs, second},
    from_request::attr::FromRequestFieldAttrs,
};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use std::{collections::HashSet, fmt, iter};
use syn::{
    parse_quote, punctuated::Punctuated, spanned::Spanned, Fields, Ident, Path, Token, Type,
};

mod attr;

#[derive(Clone, Copy)]
pub(crate) enum Trait {
    FromRequest,
    FromRequestParts,
}

impl Trait {
    fn via_marker_type(&self) -> Option<Type> {
        match self {
            Trait::FromRequest => Some(parse_quote!(M)),
            Trait::FromRequestParts => None,
        }
    }
}

impl fmt::Display for Trait {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Trait::FromRequest => f.write_str("FromRequest"),
            Trait::FromRequestParts => f.write_str("FromRequestParts"),
        }
    }
}

#[derive(Debug)]
enum State {
    Custom(syn::Type),
    Default(syn::Type),
    CannotInfer,
}

impl State {
    /// ```not_rust
    /// impl<T> A for B {}
    ///      ^ this type
    /// ```
    fn impl_generics(&self) -> impl Iterator<Item = Type> {
        match self {
            State::Default(inner) => Some(inner.clone()),
            State::Custom(_) => None,
            State::CannotInfer => Some(parse_quote!(S)),
        }
        .into_iter()
    }

    /// ```not_rust
    /// impl<T> A<T> for B {}
    ///           ^ this type
    /// ```
    fn trait_generics(&self) -> impl Iterator<Item = Type> {
        match self {
            State::Default(inner) | State::Custom(inner) => iter::once(inner.clone()),
            State::CannotInfer => iter::once(parse_quote!(S)),
        }
    }

    fn bounds(&self) -> TokenStream {
        match self {
            State::Custom(_) => quote! {},
            State::Default(inner) => quote! {
                #inner: ::std::marker::Send + ::std::marker::Sync,
            },
            State::CannotInfer => quote! {
                S: ::std::marker::Send + ::std::marker::Sync,
            },
        }
    }
}

impl ToTokens for State {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            State::Custom(inner) | State::Default(inner) => inner.to_tokens(tokens),
            State::CannotInfer => quote! { S }.to_tokens(tokens),
        }
    }
}

pub(crate) fn expand(item: syn::Item, tr: Trait) -> syn::Result<TokenStream> {
    match item {
        syn::Item::Struct(item) => {
            let syn::ItemStruct {
                attrs,
                ident,
                generics,
                fields,
                semi_token: _,
                vis: _,
                struct_token: _,
            } = item;

            let generic_ident = parse_single_generic_type_on_struct(generics, &fields, tr)?;

            let FromRequestContainerAttrs {
                via,
                rejection,
                state,
            } = parse_attrs("from_request", &attrs)?;

            let state = match state {
                Some((_, state)) => State::Custom(state),
                None => {
                    let mut inferred_state_types: HashSet<_> =
                        infer_state_type_from_field_types(&fields)
                            .chain(infer_state_type_from_field_attributes(&fields))
                            .collect();

                    if let Some((_, via)) = &via {
                        inferred_state_types.extend(state_from_via(&ident, via));
                    }

                    match inferred_state_types.len() {
                        0 => State::Default(syn::parse_quote!(S)),
                        1 => State::Custom(inferred_state_types.iter().next().unwrap().to_owned()),
                        _ => State::CannotInfer,
                    }
                }
            };

            let trait_impl = match (via.map(second), rejection.map(second)) {
                (Some(via), rejection) => impl_struct_by_extracting_all_at_once(
                    ident,
                    fields,
                    via,
                    rejection,
                    generic_ident,
                    &state,
                    tr,
                )?,
                (None, rejection) => {
                    error_on_generic_ident(generic_ident, tr)?;
                    impl_struct_by_extracting_each_field(ident, fields, rejection, &state, tr)?
                }
            };

            if let State::CannotInfer = state {
                let attr_name = match tr {
                    Trait::FromRequest => "from_request",
                    Trait::FromRequestParts => "from_request_parts",
                };
                let compile_error = syn::Error::new(
                    Span::call_site(),
                    format_args!(
                        "can't infer state type, please add \
                         `#[{attr_name}(state = MyStateType)]` attribute",
                    ),
                )
                .into_compile_error();

                Ok(quote! {
                    #trait_impl
                    #compile_error
                })
            } else {
                Ok(trait_impl)
            }
        }
        syn::Item::Enum(item) => {
            let syn::ItemEnum {
                attrs,
                vis: _,
                enum_token: _,
                ident,
                generics,
                brace_token: _,
                variants,
            } = item;

            let generics_error = format!("`#[derive({tr})]` on enums don't support generics");

            if !generics.params.is_empty() {
                return Err(syn::Error::new_spanned(generics, generics_error));
            }

            if let Some(where_clause) = generics.where_clause {
                return Err(syn::Error::new_spanned(where_clause, generics_error));
            }

            let FromRequestContainerAttrs {
                via,
                rejection,
                state,
            } = parse_attrs("from_request", &attrs)?;

            let state = match state {
                Some((_, state)) => State::Custom(state),
                None => (|| {
                    let via = via.as_ref().map(|(_, via)| via)?;
                    state_from_via(&ident, via).map(State::Custom)
                })()
                .unwrap_or_else(|| State::Default(syn::parse_quote!(S))),
            };

            match (via.map(second), rejection) {
                (Some(via), rejection) => impl_enum_by_extracting_all_at_once(
                    ident,
                    variants,
                    via,
                    rejection.map(second),
                    state,
                    tr,
                ),
                (None, Some((rejection_kw, _))) => Err(syn::Error::new_spanned(
                    rejection_kw,
                    "cannot use `rejection` without `via`",
                )),
                (None, _) => Err(syn::Error::new(
                    Span::call_site(),
                    "missing `#[from_request(via(...))]`",
                )),
            }
        }
        _ => Err(syn::Error::new_spanned(item, "expected `struct` or `enum`")),
    }
}

fn parse_single_generic_type_on_struct(
    generics: syn::Generics,
    fields: &syn::Fields,
    tr: Trait,
) -> syn::Result<Option<Ident>> {
    if let Some(where_clause) = generics.where_clause {
        return Err(syn::Error::new_spanned(
            where_clause,
            format_args!("#[derive({tr})] doesn't support structs with `where` clauses"),
        ));
    }

    match generics.params.len() {
        0 => Ok(None),
        1 => {
            let param = generics.params.first().unwrap();
            let ty_ident = match param {
                syn::GenericParam::Type(ty) => &ty.ident,
                syn::GenericParam::Lifetime(lifetime) => {
                    return Err(syn::Error::new_spanned(
                        lifetime,
                        format_args!(
                            "#[derive({tr})] doesn't support structs \
                             that are generic over lifetimes"
                        ),
                    ));
                }
                syn::GenericParam::Const(konst) => {
                    return Err(syn::Error::new_spanned(
                        konst,
                        format_args!(
                            "#[derive({tr})] doesn't support structs \
                             that have const generics"
                        ),
                    ));
                }
            };

            match fields {
                syn::Fields::Named(fields_named) => {
                    return Err(syn::Error::new_spanned(
                        fields_named,
                        format_args!(
                            "#[derive({tr})] doesn't support named fields \
                             for generic structs. Use a tuple struct instead"
                        ),
                    ));
                }
                syn::Fields::Unnamed(fields_unnamed) => {
                    if fields_unnamed.unnamed.len() != 1 {
                        return Err(syn::Error::new_spanned(
                            fields_unnamed,
                            format_args!(
                                "#[derive({tr})] only supports generics on \
                                 tuple structs that have exactly one field"
                            ),
                        ));
                    }

                    let field = fields_unnamed.unnamed.first().unwrap();

                    if let syn::Type::Path(type_path) = &field.ty {
                        if type_path.path.get_ident() != Some(ty_ident) {
                            return Err(syn::Error::new_spanned(
                                type_path,
                                format_args!(
                                    "#[derive({tr})] only supports generics on \
                                     tuple structs that have exactly one field of the generic type"
                                ),
                            ));
                        }
                    } else {
                        return Err(syn::Error::new_spanned(&field.ty, "Expected type path"));
                    }
                }
                syn::Fields::Unit => return Ok(None),
            }

            Ok(Some(ty_ident.clone()))
        }
        _ => Err(syn::Error::new_spanned(
            generics,
            format_args!("#[derive({tr})] only supports 0 or 1 generic type parameters"),
        )),
    }
}

fn error_on_generic_ident(generic_ident: Option<Ident>, tr: Trait) -> syn::Result<()> {
    if let Some(generic_ident) = generic_ident {
        Err(syn::Error::new_spanned(
            generic_ident,
            format_args!(
                "#[derive({tr})] only supports generics when used with #[from_request(via)]"
            ),
        ))
    } else {
        Ok(())
    }
}

fn impl_struct_by_extracting_each_field(
    ident: syn::Ident,
    fields: syn::Fields,
    rejection: Option<syn::Path>,
    state: &State,
    tr: Trait,
) -> syn::Result<TokenStream> {
    let trait_fn_body = match state {
        State::CannotInfer => quote! {
            ::std::unimplemented!()
        },
        _ => {
            let extract_fields = extract_fields(&fields, &rejection, tr)?;
            quote! {
                ::std::result::Result::Ok(Self {
                    #(#extract_fields)*
                })
            }
        }
    };

    let rejection_ident = if let Some(rejection) = rejection {
        quote!(#rejection)
    } else if has_no_fields(&fields) {
        quote!(::std::convert::Infallible)
    } else {
        quote!(::axum::response::Response)
    };

    let impl_generics = state
        .impl_generics()
        .collect::<Punctuated<Type, Token![,]>>();

    let trait_generics = state
        .trait_generics()
        .collect::<Punctuated<Type, Token![,]>>();

    let state_bounds = state.bounds();

    Ok(match tr {
        Trait::FromRequest => quote! {
            #[automatically_derived]
            impl<#impl_generics> ::axum::extract::FromRequest<#trait_generics> for #ident
            where
                #state_bounds
            {
                type Rejection = #rejection_ident;

                async fn from_request(
                    mut req: ::axum::http::Request<::axum::body::Body>,
                    state: &#state,
                ) -> ::std::result::Result<Self, Self::Rejection> {
                    #trait_fn_body
                }
            }
        },
        Trait::FromRequestParts => quote! {
            #[automatically_derived]
            impl<#impl_generics> ::axum::extract::FromRequestParts<#trait_generics> for #ident
            where
                #state_bounds
            {
                type Rejection = #rejection_ident;

                async fn from_request_parts(
                    parts: &mut ::axum::http::request::Parts,
                    state: &#state,
                ) -> ::std::result::Result<Self, Self::Rejection> {
                    #trait_fn_body
                }
            }
        },
    })
}

fn has_no_fields(fields: &syn::Fields) -> bool {
    match fields {
        syn::Fields::Named(fields) => fields.named.is_empty(),
        syn::Fields::Unnamed(fields) => fields.unnamed.is_empty(),
        syn::Fields::Unit => true,
    }
}

fn extract_fields(
    fields: &syn::Fields,
    rejection: &Option<syn::Path>,
    tr: Trait,
) -> syn::Result<Vec<TokenStream>> {
    fn member(field: &syn::Field, index: usize) -> TokenStream {
        match &field.ident {
            Some(ident) => quote! { #ident },
            _ => {
                let member = syn::Member::Unnamed(syn::Index {
                    index: index as u32,
                    span: field.span(),
                });
                quote! { #member }
            }
        }
    }

    fn into_inner(via: &Option<(attr::kw::via, syn::Path)>, ty_span: Span) -> TokenStream {
        if let Some((_, path)) = via {
            let span = path.span();
            quote_spanned! {span=>
                |#path(inner)| inner
            }
        } else {
            quote_spanned! {ty_span=>
                ::std::convert::identity
            }
        }
    }

    fn into_outer(
        via: &Option<(attr::kw::via, syn::Path)>,
        ty_span: Span,
        field_ty: &Type,
    ) -> TokenStream {
        if let Some((_, path)) = via {
            let span = path.span();
            quote_spanned! {span=>
                #path<#field_ty>
            }
        } else {
            quote_spanned! {ty_span=>
                #field_ty
            }
        }
    }

    let mut fields_iter = fields.iter();

    let last = match tr {
        // Use FromRequestParts for all elements except the last
        Trait::FromRequest => fields_iter.next_back(),
        // Use FromRequestParts for all elements
        Trait::FromRequestParts => None,
    };

    let mut res: Vec<_> = fields_iter
        .enumerate()
        .map(|(index, field)| {
            let FromRequestFieldAttrs { via } = parse_attrs("from_request", &field.attrs)?;

            let member = member(field, index);
            let ty_span = field.ty.span();
            let into_inner = into_inner(&via, ty_span);

            if peel_option(&field.ty).is_some() {
                let field_ty = into_outer(&via, ty_span, peel_option(&field.ty).unwrap());
                let tokens = match tr {
                    Trait::FromRequest => {
                        quote_spanned! {ty_span=>
                            #member: {
                                let (mut parts, body) = req.into_parts();
                                let value =
                                    <#field_ty as ::axum::extract::FromRequestParts<_>>::from_request_parts(
                                        &mut parts,
                                        state,
                                    )
                                    .await
                                    .ok()
                                    .map(#into_inner);
                                req = ::axum::http::Request::from_parts(parts, body);
                                value
                            },
                        }
                    }
                    Trait::FromRequestParts => {
                        quote_spanned! {ty_span=>
                            #member: {
                                <#field_ty as ::axum::extract::FromRequestParts<_>>::from_request_parts(
                                    parts,
                                    state,
                                )
                                .await
                                .ok()
                                .map(#into_inner)
                            },
                        }
                    }
                };
                Ok(tokens)
            } else if peel_result_ok(&field.ty).is_some() {
                let field_ty = into_outer(&via,ty_span, peel_result_ok(&field.ty).unwrap());
                let tokens = match tr {
                    Trait::FromRequest => {
                        quote_spanned! {ty_span=>
                            #member: {
                                let (mut parts, body) = req.into_parts();
                                let value =
                                    <#field_ty as ::axum::extract::FromRequestParts<_>>::from_request_parts(
                                        &mut parts,
                                        state,
                                    )
                                    .await
                                    .map(#into_inner);
                                req = ::axum::http::Request::from_parts(parts, body);
                                value
                            },
                        }
                    }
                    Trait::FromRequestParts => {
                        quote_spanned! {ty_span=>
                            #member: {
                                <#field_ty as ::axum::extract::FromRequestParts<_>>::from_request_parts(
                                    parts,
                                    state,
                                )
                                .await
                                .map(#into_inner)
                            },
                        }
                    }
                };
                Ok(tokens)
            } else {
                let field_ty = into_outer(&via,ty_span,&field.ty);
                let map_err = if let Some(rejection) = rejection {
                    quote! { <#rejection as ::std::convert::From<_>>::from }
                } else {
                    quote! { ::axum::response::IntoResponse::into_response }
                };

                let tokens = match tr {
                    Trait::FromRequest => {
                        quote_spanned! {ty_span=>
                            #member: {
                                let (mut parts, body) = req.into_parts();
                                let value =
                                    <#field_ty as ::axum::extract::FromRequestParts<_>>::from_request_parts(
                                        &mut parts,
                                        state,
                                    )
                                    .await
                                    .map(#into_inner)
                                    .map_err(#map_err)?;
                                req = ::axum::http::Request::from_parts(parts, body);
                                value
                            },
                        }
                    }
                    Trait::FromRequestParts => {
                        quote_spanned! {ty_span=>
                            #member: {
                                <#field_ty as ::axum::extract::FromRequestParts<_>>::from_request_parts(
                                    parts,
                                    state,
                                )
                                .await
                                .map(#into_inner)
                                .map_err(#map_err)?
                            },
                        }
                    }
                };
                Ok(tokens)
            }
        })
        .collect::<syn::Result<_>>()?;

    // Handle the last element, if deriving FromRequest
    if let Some(field) = last {
        let FromRequestFieldAttrs { via } = parse_attrs("from_request", &field.attrs)?;

        let member = member(field, fields.len() - 1);
        let ty_span = field.ty.span();
        let into_inner = into_inner(&via, ty_span);

        let item = if peel_option(&field.ty).is_some() {
            let field_ty = into_outer(&via, ty_span, peel_option(&field.ty).unwrap());
            quote_spanned! {ty_span=>
                #member: {
                    <#field_ty as ::axum::extract::FromRequest<_, _>>::from_request(req, state)
                        .await
                        .ok()
                        .map(#into_inner)
                },
            }
        } else if peel_result_ok(&field.ty).is_some() {
            let field_ty = into_outer(&via, ty_span, peel_result_ok(&field.ty).unwrap());
            quote_spanned! {ty_span=>
                #member: {
                    <#field_ty as ::axum::extract::FromRequest<_, _>>::from_request(req, state)
                        .await
                        .map(#into_inner)
                },
            }
        } else {
            let field_ty = into_outer(&via, ty_span, &field.ty);
            let map_err = if let Some(rejection) = rejection {
                quote! { <#rejection as ::std::convert::From<_>>::from }
            } else {
                quote! { ::axum::response::IntoResponse::into_response }
            };

            quote_spanned! {ty_span=>
                #member: {
                    <#field_ty as ::axum::extract::FromRequest<_, _>>::from_request(req, state)
                        .await
                        .map(#into_inner)
                        .map_err(#map_err)?
                },
            }
        };

        res.push(item);
    }

    Ok(res)
}

fn peel_option(ty: &syn::Type) -> Option<&syn::Type> {
    let type_path = if let syn::Type::Path(type_path) = ty {
        type_path
    } else {
        return None;
    };

    let segment = type_path.path.segments.last()?;

    if segment.ident != "Option" {
        return None;
    }

    let args = match &segment.arguments {
        syn::PathArguments::AngleBracketed(args) => args,
        syn::PathArguments::Parenthesized(_) | syn::PathArguments::None => return None,
    };

    let ty = if args.args.len() == 1 {
        args.args.last().unwrap()
    } else {
        return None;
    };

    if let syn::GenericArgument::Type(ty) = ty {
        Some(ty)
    } else {
        None
    }
}

fn peel_result_ok(ty: &syn::Type) -> Option<&syn::Type> {
    let type_path = if let syn::Type::Path(type_path) = ty {
        type_path
    } else {
        return None;
    };

    let segment = type_path.path.segments.last()?;

    if segment.ident != "Result" {
        return None;
    }

    let args = match &segment.arguments {
        syn::PathArguments::AngleBracketed(args) => args,
        syn::PathArguments::Parenthesized(_) | syn::PathArguments::None => return None,
    };

    let ty = if args.args.len() == 2 {
        args.args.first().unwrap()
    } else {
        return None;
    };

    if let syn::GenericArgument::Type(ty) = ty {
        Some(ty)
    } else {
        None
    }
}

fn impl_struct_by_extracting_all_at_once(
    ident: syn::Ident,
    fields: syn::Fields,
    via_path: syn::Path,
    rejection: Option<syn::Path>,
    generic_ident: Option<Ident>,
    state: &State,
    tr: Trait,
) -> syn::Result<TokenStream> {
    let fields = match fields {
        syn::Fields::Named(fields) => fields.named.into_iter(),
        syn::Fields::Unnamed(fields) => fields.unnamed.into_iter(),
        syn::Fields::Unit => Punctuated::<_, Token![,]>::new().into_iter(),
    };

    for field in fields {
        let FromRequestFieldAttrs { via } = parse_attrs("from_request", &field.attrs)?;

        if let Some((via, _)) = via {
            return Err(syn::Error::new_spanned(
                via,
                "`#[from_request(via(...))]` on a field cannot be used \
                together with `#[from_request(...)]` on the container",
            ));
        }
    }

    let path_span = via_path.span();

    let (associated_rejection_type, map_err) = if let Some(rejection) = &rejection {
        let rejection = quote! { #rejection };
        let map_err = quote! { ::std::convert::From::from };
        (rejection, map_err)
    } else {
        let rejection = quote! {
            ::axum::response::Response
        };
        let map_err = quote! { ::axum::response::IntoResponse::into_response };
        (rejection, map_err)
    };

    // for something like
    //
    // ```
    // #[derive(Clone, Default, FromRequest)]
    // #[from_request(via(State))]
    // struct AppState {}
    // ```
    //
    // we need to implement `impl<M> FromRequest<AppState, M>` but only for
    // - `#[derive(FromRequest)]`, not `#[derive(FromRequestParts)]`
    // - `State`, not other extractors
    //
    // honestly not sure why but the tests all pass
    let via_marker_type = if path_ident_is_state(&via_path) {
        tr.via_marker_type()
    } else {
        None
    };

    let impl_generics = via_marker_type
        .iter()
        .cloned()
        .chain(state.impl_generics())
        .chain(generic_ident.is_some().then(|| parse_quote!(T)))
        .collect::<Punctuated<Type, Token![,]>>();

    let trait_generics = state
        .trait_generics()
        .chain(via_marker_type)
        .collect::<Punctuated<Type, Token![,]>>();

    let ident_generics = generic_ident
        .is_some()
        .then(|| quote! { <T> })
        .unwrap_or_default();

    let rejection_bound = rejection.as_ref().map(|rejection| {
        match (tr, generic_ident.is_some()) {
            (Trait::FromRequest, true) => {
                quote! {
                    #rejection: ::std::convert::From<<#via_path<T> as ::axum::extract::FromRequest<#trait_generics>>::Rejection>,
                }
            },
            (Trait::FromRequest, false) => {
                quote! {
                    #rejection: ::std::convert::From<<#via_path<Self> as ::axum::extract::FromRequest<#trait_generics>>::Rejection>,
                }
            },
            (Trait::FromRequestParts, true) => {
                quote! {
                    #rejection: ::std::convert::From<<#via_path<T> as ::axum::extract::FromRequestParts<#trait_generics>>::Rejection>,
                }
            },
            (Trait::FromRequestParts, false) => {
                quote! {
                    #rejection: ::std::convert::From<<#via_path<Self> as ::axum::extract::FromRequestParts<#trait_generics>>::Rejection>,
                }
            }
        }
    }).unwrap_or_default();

    let via_type_generics = if generic_ident.is_some() {
        quote! { T }
    } else {
        quote! { Self }
    };

    let value_to_self = if generic_ident.is_some() {
        quote! {
            #ident(value)
        }
    } else {
        quote! { value }
    };

    let state_bounds = state.bounds();

    let tokens = match tr {
        Trait::FromRequest => {
            quote_spanned! {path_span=>
                #[automatically_derived]
                impl<#impl_generics> ::axum::extract::FromRequest<#trait_generics> for #ident #ident_generics
                where
                    #via_path<#via_type_generics>: ::axum::extract::FromRequest<#trait_generics>,
                    #rejection_bound
                    #state_bounds
                {
                    type Rejection = #associated_rejection_type;

                    async fn from_request(
                        req: ::axum::http::Request<::axum::body::Body>,
                        state: &#state,
                    ) -> ::std::result::Result<Self, Self::Rejection> {
                        <#via_path<#via_type_generics> as ::axum::extract::FromRequest<_, _>>::from_request(req, state)
                            .await
                            .map(|#via_path(value)| #value_to_self)
                            .map_err(#map_err)
                    }
                }
            }
        }
        Trait::FromRequestParts => {
            quote_spanned! {path_span=>
                #[automatically_derived]
                impl<#impl_generics> ::axum::extract::FromRequestParts<#trait_generics> for #ident #ident_generics
                where
                    #via_path<#via_type_generics>: ::axum::extract::FromRequestParts<#trait_generics>,
                    #rejection_bound
                    #state_bounds
                {
                    type Rejection = #associated_rejection_type;

                    async fn from_request_parts(
                        parts: &mut ::axum::http::request::Parts,
                        state: &#state,
                    ) -> ::std::result::Result<Self, Self::Rejection> {
                        <#via_path<#via_type_generics> as ::axum::extract::FromRequestParts<_>>::from_request_parts(parts, state)
                            .await
                            .map(|#via_path(value)| #value_to_self)
                            .map_err(#map_err)
                    }
                }
            }
        }
    };

    Ok(tokens)
}

fn impl_enum_by_extracting_all_at_once(
    ident: syn::Ident,
    variants: Punctuated<syn::Variant, Token![,]>,
    path: syn::Path,
    rejection: Option<syn::Path>,
    state: State,
    tr: Trait,
) -> syn::Result<TokenStream> {
    for variant in variants {
        let FromRequestFieldAttrs { via } = parse_attrs("from_request", &variant.attrs)?;

        if let Some((via, _)) = via {
            return Err(syn::Error::new_spanned(
                via,
                "`#[from_request(via(...))]` cannot be used on variants",
            ));
        }

        let fields = match variant.fields {
            syn::Fields::Named(fields) => fields.named.into_iter(),
            syn::Fields::Unnamed(fields) => fields.unnamed.into_iter(),
            syn::Fields::Unit => Punctuated::<_, Token![,]>::new().into_iter(),
        };

        for field in fields {
            let FromRequestFieldAttrs { via } = parse_attrs("from_request", &field.attrs)?;
            if let Some((via, _)) = via {
                return Err(syn::Error::new_spanned(
                    via,
                    "`#[from_request(via(...))]` cannot be used inside variants",
                ));
            }
        }
    }

    let (associated_rejection_type, map_err) = if let Some(rejection) = &rejection {
        let rejection = quote! { #rejection };
        let map_err = quote! { ::std::convert::From::from };
        (rejection, map_err)
    } else {
        let rejection = quote! {
            ::axum::response::Response
        };
        let map_err = quote! { ::axum::response::IntoResponse::into_response };
        (rejection, map_err)
    };

    let path_span = path.span();

    let impl_generics = state
        .impl_generics()
        .collect::<Punctuated<Type, Token![,]>>();

    let trait_generics = state
        .trait_generics()
        .collect::<Punctuated<Type, Token![,]>>();

    let state_bounds = state.bounds();

    let tokens = match tr {
        Trait::FromRequest => {
            quote_spanned! {path_span=>
                #[automatically_derived]
                impl<#impl_generics> ::axum::extract::FromRequest<#trait_generics> for #ident
                where
                    #state_bounds
                {
                    type Rejection = #associated_rejection_type;

                    async fn from_request(
                        req: ::axum::http::Request<::axum::body::Body>,
                        state: &#state,
                    ) -> ::std::result::Result<Self, Self::Rejection> {
                        <#path::<#ident> as ::axum::extract::FromRequest<_, _>>::from_request(req, state)
                            .await
                            .map(|#path(inner)| inner)
                            .map_err(#map_err)
                    }
                }
            }
        }
        Trait::FromRequestParts => {
            quote_spanned! {path_span=>
                #[automatically_derived]
                impl<#impl_generics> ::axum::extract::FromRequestParts<#trait_generics> for #ident
                where
                    #state_bounds
                {
                    type Rejection = #associated_rejection_type;

                    async fn from_request_parts(
                        parts: &mut ::axum::http::request::Parts,
                        state: &#state,
                    ) -> ::std::result::Result<Self, Self::Rejection> {
                        <#path::<#ident> as ::axum::extract::FromRequestParts<_>>::from_request_parts(parts, state)
                            .await
                            .map(|#path(inner)| inner)
                            .map_err(#map_err)
                    }
                }
            }
        }
    };

    Ok(tokens)
}

/// For a struct like
///
/// ```skip
/// struct Extractor {
///     state: State<AppState>,
/// }
/// ```
///
/// We can infer the state type to be `AppState` because it appears inside a `State`
fn infer_state_type_from_field_types(fields: &Fields) -> impl Iterator<Item = Type> + '_ {
    match fields {
        Fields::Named(fields_named) => Box::new(crate::infer_state_types(
            fields_named.named.iter().map(|field| &field.ty),
        )) as Box<dyn Iterator<Item = Type>>,
        Fields::Unnamed(fields_unnamed) => Box::new(crate::infer_state_types(
            fields_unnamed.unnamed.iter().map(|field| &field.ty),
        )),
        Fields::Unit => Box::new(iter::empty()),
    }
}

/// For a struct like
///
/// ```skip
/// struct Extractor {
///     #[from_request(via(State))]
///     state: AppState,
/// }
/// ```
///
/// We can infer the state type to be `AppState` because it has `via(State)` and thus can be
/// extracted with `State<AppState>`
fn infer_state_type_from_field_attributes(fields: &Fields) -> impl Iterator<Item = Type> + '_ {
    match fields {
        Fields::Named(fields_named) => {
            Box::new(fields_named.named.iter().filter_map(|field| {
                // TODO(david): it's a little wasteful to parse the attributes again here
                // ideally we should parse things once and pass the data down
                let FromRequestFieldAttrs { via } =
                    parse_attrs("from_request", &field.attrs).ok()?;
                let (_, via_path) = via?;
                path_ident_is_state(&via_path).then(|| field.ty.clone())
            })) as Box<dyn Iterator<Item = Type>>
        }
        Fields::Unnamed(fields_unnamed) => {
            Box::new(fields_unnamed.unnamed.iter().filter_map(|field| {
                // TODO(david): it's a little wasteful to parse the attributes again here
                // ideally we should parse things once and pass the data down
                let FromRequestFieldAttrs { via } =
                    parse_attrs("from_request", &field.attrs).ok()?;
                let (_, via_path) = via?;
                path_ident_is_state(&via_path).then(|| field.ty.clone())
            }))
        }
        Fields::Unit => Box::new(iter::empty()),
    }
}

fn path_ident_is_state(path: &Path) -> bool {
    if let Some(last_segment) = path.segments.last() {
        last_segment.ident == "State"
    } else {
        false
    }
}

fn state_from_via(ident: &Ident, via: &Path) -> Option<Type> {
    path_ident_is_state(via).then(|| parse_quote!(#ident))
}

#[test]
fn ui() {
    crate::run_ui_tests("from_request");
}

/// For some reason the compiler error for this is different locally and on CI. No idea why... So
/// we don't use trybuild for this test.
///
/// ```compile_fail
/// #[derive(axum_macros::FromRequest)]
/// struct Extractor {
///     thing: bool,
/// }
/// ```
#[allow(dead_code)]
fn test_field_doesnt_impl_from_request() {}
