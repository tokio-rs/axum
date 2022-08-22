use self::attr::{
    parse_container_attrs, parse_field_attrs, FromRequestContainerAttr, FromRequestFieldAttr,
};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{punctuated::Punctuated, spanned::Spanned, Ident, Token};

mod attr;

pub(crate) fn expand(item: syn::Item) -> syn::Result<TokenStream> {
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

            let generic_ident = parse_single_generic_type_on_struct(generics, &fields)?;

            match parse_container_attrs(&attrs)? {
                FromRequestContainerAttr::Via { path, rejection } => {
                    impl_struct_by_extracting_all_at_once(
                        ident,
                        fields,
                        path,
                        rejection,
                        generic_ident,
                    )
                }
                FromRequestContainerAttr::Rejection(rejection) => {
                    error_on_generic_ident(generic_ident)?;

                    impl_struct_by_extracting_each_field(ident, fields, Some(rejection))
                }
                FromRequestContainerAttr::None => {
                    error_on_generic_ident(generic_ident)?;

                    impl_struct_by_extracting_each_field(ident, fields, None)
                }
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

            const GENERICS_ERROR: &str = "`#[derive(FromRequest)] on enums don't support generics";

            if !generics.params.is_empty() {
                return Err(syn::Error::new_spanned(generics, GENERICS_ERROR));
            }

            if let Some(where_clause) = generics.where_clause {
                return Err(syn::Error::new_spanned(where_clause, GENERICS_ERROR));
            }

            match parse_container_attrs(&attrs)? {
                FromRequestContainerAttr::Via { path, rejection } => {
                    impl_enum_by_extracting_all_at_once(ident, variants, path, rejection)
                }
                FromRequestContainerAttr::Rejection(rejection) => Err(syn::Error::new_spanned(
                    rejection,
                    "cannot use `rejection` without `via`",
                )),
                FromRequestContainerAttr::None => Err(syn::Error::new(
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
) -> syn::Result<Option<Ident>> {
    if let Some(where_clause) = generics.where_clause {
        return Err(syn::Error::new_spanned(
            where_clause,
            "#[derive(FromRequest)] doesn't support structs with `where` clauses",
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
                        "#[derive(FromRequest)] doesn't support structs that are generic over lifetimes",
                    ));
                }
                syn::GenericParam::Const(konst) => {
                    return Err(syn::Error::new_spanned(
                        konst,
                        "#[derive(FromRequest)] doesn't support structs that have const generics",
                    ));
                }
            };

            match fields {
                syn::Fields::Named(fields_named) => {
                    return Err(syn::Error::new_spanned(
                        fields_named,
                        "#[derive(FromRequest)] doesn't support named fields for generic structs. Use a tuple struct instead",
                    ));
                }
                syn::Fields::Unnamed(fields_unnamed) => {
                    if fields_unnamed.unnamed.len() != 1 {
                        return Err(syn::Error::new_spanned(
                            fields_unnamed,
                            "#[derive(FromRequest)] only supports generics on tuple structs that have exactly one field",
                        ));
                    }

                    let field = fields_unnamed.unnamed.first().unwrap();

                    if let syn::Type::Path(type_path) = &field.ty {
                        if type_path
                            .path
                            .get_ident()
                            .map_or(true, |field_type_ident| field_type_ident != ty_ident)
                        {
                            return Err(syn::Error::new_spanned(
                                type_path,
                                "#[derive(FromRequest)] only supports generics on tuple structs that have exactly one field of the generic type",
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
            "#[derive(FromRequest)] only supports 0 or 1 generic type parameters",
        )),
    }
}

fn error_on_generic_ident(generic_ident: Option<Ident>) -> syn::Result<()> {
    if let Some(generic_ident) = generic_ident {
        Err(syn::Error::new_spanned(
            generic_ident,
            "#[derive(FromRequest)] only supports generics when used with #[from_request(via)]",
        ))
    } else {
        Ok(())
    }
}

fn impl_struct_by_extracting_each_field(
    ident: syn::Ident,
    fields: syn::Fields,
    rejection: Option<syn::Path>,
) -> syn::Result<TokenStream> {
    let extract_fields = extract_fields(&fields, &rejection)?;

    let rejection_ident = if let Some(rejection) = rejection {
        quote!(#rejection)
    } else if has_no_fields(&fields) {
        quote!(::std::convert::Infallible)
    } else {
        quote!(::axum::response::Response)
    };

    Ok(quote! {
        #[::axum::async_trait]
        #[automatically_derived]
        impl<S, B> ::axum::extract::FromRequest<S, B> for #ident
        where
            B: ::axum::body::HttpBody + ::std::marker::Send + 'static,
            B::Data: ::std::marker::Send,
            B::Error: ::std::convert::Into<::axum::BoxError>,
            S: ::std::marker::Send + ::std::marker::Sync,
        {
            type Rejection = #rejection_ident;

            async fn from_request(
                mut req: axum::http::Request<B>,
                state: &S,
            ) -> ::std::result::Result<Self, Self::Rejection> {
                ::std::result::Result::Ok(Self {
                    #(#extract_fields)*
                })
            }
        }
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
) -> syn::Result<Vec<TokenStream>> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let is_last = fields.len() - 1 == index;

            let FromRequestFieldAttr { via } = parse_field_attrs(&field.attrs)?;

            let member = if let Some(ident) = &field.ident {
                quote! { #ident }
            } else {
                let member = syn::Member::Unnamed(syn::Index {
                    index: index as u32,
                    span: field.span(),
                });
                quote! { #member }
            };

            let ty_span = field.ty.span();

            let into_inner = if let Some((_, path)) = via {
                let span = path.span();
                quote_spanned! {span=>
                    |#path(inner)| inner
                }
            } else {
                quote_spanned! {ty_span=>
                    ::std::convert::identity
                }
            };

            if peel_option(&field.ty).is_some() {
                if is_last {
                    Ok(quote_spanned! {ty_span=>
                        #member: {
                            ::axum::extract::FromRequest::from_request(req, state)
                                .await
                                .ok()
                                .map(#into_inner)
                        },
                    })
                } else {
                    Ok(quote_spanned! {ty_span=>
                        #member: {
                            let (mut parts, body) = req.into_parts();
                            let value = ::axum::extract::FromRequestParts::from_request_parts(&mut parts, state)
                                .await
                                .ok()
                                .map(#into_inner);
                            req = ::axum::http::Request::from_parts(parts, body);
                            value
                        },
                    })
                }
            } else if peel_result_ok(&field.ty).is_some() {
                if is_last {
                    Ok(quote_spanned! {ty_span=>
                        #member: {
                            ::axum::extract::FromRequest::from_request(req, state)
                                .await
                                .map(#into_inner)
                        },
                    })
                } else {
                    Ok(quote_spanned! {ty_span=>
                        #member: {
                            let (mut parts, body) = req.into_parts();
                            let value = ::axum::extract::FromRequestParts::from_request_parts(&mut parts, state)
                                .await
                                .map(#into_inner);
                            req = ::axum::http::Request::from_parts(parts, body);
                            value
                        },
                    })
                }
            } else {
                let map_err = if let Some(rejection) = rejection {
                    quote! { <#rejection as ::std::convert::From<_>>::from }
                } else {
                    quote! { ::axum::response::IntoResponse::into_response }
                };

                if is_last {
                    Ok(quote_spanned! {ty_span=>
                        #member: {
                            ::axum::extract::FromRequest::from_request(req, state)
                                .await
                                .map(#into_inner)
                                .map_err(#map_err)?
                        },
                    })
                } else {
                    Ok(quote_spanned! {ty_span=>
                        #member: {
                            let (mut parts, body) = req.into_parts();
                            let value = ::axum::extract::FromRequestParts::from_request_parts(&mut parts, state)
                                .await
                                .map(#into_inner)
                                .map_err(#map_err)?;
                            req = ::axum::http::Request::from_parts(parts, body);
                            value
                        },
                    })
                }
            }
        })
        .collect()
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
    path: syn::Path,
    rejection: Option<syn::Path>,
    generic_ident: Option<Ident>,
) -> syn::Result<TokenStream> {
    let fields = match fields {
        syn::Fields::Named(fields) => fields.named.into_iter(),
        syn::Fields::Unnamed(fields) => fields.unnamed.into_iter(),
        syn::Fields::Unit => Punctuated::<_, Token![,]>::new().into_iter(),
    };

    for field in fields {
        let FromRequestFieldAttr { via } = parse_field_attrs(&field.attrs)?;
        if let Some((via, _)) = via {
            return Err(syn::Error::new_spanned(
                via,
                "`#[from_request(via(...))]` on a field cannot be used \
                together with `#[from_request(...)]` on the container",
            ));
        }
    }

    let path_span = path.span();

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

    let rejection_bound = rejection.as_ref().map(|rejection| {
        if generic_ident.is_some() {
            quote! {
                #rejection: ::std::convert::From<<#path<T> as ::axum::extract::FromRequest<S, B>>::Rejection>,
            }
        } else {
            quote! {
                #rejection: ::std::convert::From<<#path<Self> as ::axum::extract::FromRequest<S, B>>::Rejection>,
            }
        }
    }).unwrap_or_default();

    let impl_generics = if generic_ident.is_some() {
        quote! { S, B, T }
    } else {
        quote! { S, B }
    };

    let type_generics = generic_ident
        .is_some()
        .then(|| quote! { <T> })
        .unwrap_or_default();

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

    Ok(quote_spanned! {path_span=>
        #[::axum::async_trait]
        #[automatically_derived]
        impl<#impl_generics> ::axum::extract::FromRequest<S, B> for #ident #type_generics
        where
            #path<#via_type_generics>: ::axum::extract::FromRequest<S, B>,
            #rejection_bound
            B: ::std::marker::Send + 'static,
            S: ::std::marker::Send + ::std::marker::Sync,
        {
            type Rejection = #associated_rejection_type;

            async fn from_request(
                req: ::axum::http::Request<B>,
                state: &S
            ) -> ::std::result::Result<Self, Self::Rejection> {
                ::axum::extract::FromRequest::from_request(req, state)
                    .await
                    .map(|#path(value)| #value_to_self)
                    .map_err(#map_err)
            }
        }
    })
}

fn impl_enum_by_extracting_all_at_once(
    ident: syn::Ident,
    variants: Punctuated<syn::Variant, Token![,]>,
    path: syn::Path,
    rejection: Option<syn::Path>,
) -> syn::Result<TokenStream> {
    for variant in variants {
        let FromRequestFieldAttr { via } = parse_field_attrs(&variant.attrs)?;
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
            let FromRequestFieldAttr { via } = parse_field_attrs(&field.attrs)?;
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

    Ok(quote_spanned! {path_span=>
        #[::axum::async_trait]
        #[automatically_derived]
        impl<S, B> ::axum::extract::FromRequest<S, B> for #ident
        where
            B: ::axum::body::HttpBody + ::std::marker::Send + 'static,
            B::Data: ::std::marker::Send,
            B::Error: ::std::convert::Into<::axum::BoxError>,
            S: ::std::marker::Send + ::std::marker::Sync,
        {
            type Rejection = #associated_rejection_type;

            async fn from_request(
                req: ::axum::http::Request<B>,
                state: &S
            ) -> ::std::result::Result<Self, Self::Rejection> {
                ::axum::extract::FromRequest::from_request(req, state)
                    .await
                    .map(|#path(inner)| inner)
                    .map_err(#map_err)
            }
        }
    })
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
