use self::attr::{
    parse_container_attrs, parse_field_attrs, FromRequestContainerAttr, FromRequestFieldAttr,
    RejectionDeriveOptOuts,
};
use heck::ToUpperCamelCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
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
                vis,
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
                FromRequestContainerAttr::RejectionDerive(_, opt_outs) => {
                    error_on_generic_ident(generic_ident)?;

                    impl_struct_by_extracting_each_field(ident, fields, vis, opt_outs, None)
                }
                FromRequestContainerAttr::Rejection(rejection) => {
                    error_on_generic_ident(generic_ident)?;

                    impl_struct_by_extracting_each_field(
                        ident,
                        fields,
                        vis,
                        RejectionDeriveOptOuts::default(),
                        Some(rejection),
                    )
                }
                FromRequestContainerAttr::None => {
                    error_on_generic_ident(generic_ident)?;

                    impl_struct_by_extracting_each_field(
                        ident,
                        fields,
                        vis,
                        RejectionDeriveOptOuts::default(),
                        None,
                    )
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
                FromRequestContainerAttr::RejectionDerive(rejection_derive, _) => {
                    Err(syn::Error::new_spanned(
                        rejection_derive,
                        "cannot use `rejection_derive` on enums",
                    ))
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
    vis: syn::Visibility,
    rejection_derive_opt_outs: RejectionDeriveOptOuts,
    rejection: Option<syn::Path>,
) -> syn::Result<TokenStream> {
    let extract_fields = extract_fields(&fields, &rejection)?;

    let (rejection_ident, rejection) = if let Some(rejection) = rejection {
        let rejection_ident = syn::parse_quote!(#rejection);
        (rejection_ident, None)
    } else if has_no_fields(&fields) {
        (syn::parse_quote!(::std::convert::Infallible), None)
    } else {
        let rejection_ident = rejection_ident(&ident);
        let rejection =
            extract_each_field_rejection(&ident, &fields, &vis, rejection_derive_opt_outs)?;
        (rejection_ident, Some(rejection))
    };

    Ok(quote! {
        #[::axum::async_trait]
        #[automatically_derived]
        impl<B, S> ::axum::extract::FromRequest<B, S> for #ident
        where
            B: ::axum::body::HttpBody + ::std::marker::Send + 'static,
            B::Data: ::std::marker::Send,
            B::Error: ::std::convert::Into<::axum::BoxError>,
            S: Send,
        {
            type Rejection = #rejection_ident;

            async fn from_request(
                req: &mut ::axum::extract::RequestParts<B, S>,
            ) -> ::std::result::Result<Self, Self::Rejection> {
                ::std::result::Result::Ok(Self {
                    #(#extract_fields)*
                })
            }
        }

        #rejection
    })
}

fn has_no_fields(fields: &syn::Fields) -> bool {
    match fields {
        syn::Fields::Named(fields) => fields.named.is_empty(),
        syn::Fields::Unnamed(fields) => fields.unnamed.is_empty(),
        syn::Fields::Unit => true,
    }
}

fn rejection_ident(ident: &syn::Ident) -> syn::Type {
    let ident = format_ident!("{}Rejection", ident);
    syn::parse_quote!(#ident)
}

fn extract_fields(
    fields: &syn::Fields,
    rejection: &Option<syn::Path>,
) -> syn::Result<Vec<TokenStream>> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
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

            let rejection_variant_name = rejection_variant_name(field)?;

            if peel_option(&field.ty).is_some() {
                Ok(quote_spanned! {ty_span=>
                    #member: {
                        ::axum::extract::FromRequest::from_request(req)
                            .await
                            .ok()
                            .map(#into_inner)
                    },
                })
            } else if peel_result_ok(&field.ty).is_some() {
                Ok(quote_spanned! {ty_span=>
                    #member: {
                        ::axum::extract::FromRequest::from_request(req)
                            .await
                            .map(#into_inner)
                    },
                })
            } else {
                let map_err = if let Some(rejection) = rejection {
                    quote! { <#rejection as ::std::convert::From<_>>::from }
                } else {
                    quote! { Self::Rejection::#rejection_variant_name }
                };

                Ok(quote_spanned! {ty_span=>
                    #member: {
                        ::axum::extract::FromRequest::from_request(req)
                            .await
                            .map(#into_inner)
                            .map_err(#map_err)?
                    },
                })
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

fn extract_each_field_rejection(
    ident: &syn::Ident,
    fields: &syn::Fields,
    vis: &syn::Visibility,
    rejection_derive_opt_outs: RejectionDeriveOptOuts,
) -> syn::Result<TokenStream> {
    let rejection_ident = rejection_ident(ident);

    let variants = fields
        .iter()
        .map(|field| {
            let FromRequestFieldAttr { via } = parse_field_attrs(&field.attrs)?;

            let field_ty = &field.ty;
            let ty_span = field_ty.span();

            let variant_name = rejection_variant_name(field)?;

            let extractor_ty = if let Some((_, path)) = via {
                if let Some(inner) = peel_option(field_ty) {
                    quote_spanned! {ty_span=>
                        ::std::option::Option<#path<#inner>>
                    }
                } else if let Some(inner) = peel_result_ok(field_ty) {
                    quote_spanned! {ty_span=>
                        ::std::result::Result<#path<#inner>, TypedHeaderRejection>
                    }
                } else {
                    quote_spanned! {ty_span=> #path<#field_ty> }
                }
            } else {
                quote_spanned! {ty_span=> #field_ty }
            };

            Ok(quote_spanned! {ty_span=>
                #[allow(non_camel_case_types)]
                #variant_name(<#extractor_ty as ::axum::extract::FromRequest<::axum::body::Body, ()>>::Rejection),
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let impl_into_response = {
        let arms = fields
            .iter()
            .map(|field| {
                let variant_name = rejection_variant_name(field)?;
                Ok(quote! {
                    Self::#variant_name(inner) => inner.into_response(),
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;

        quote! {
            #[automatically_derived]
            impl ::axum::response::IntoResponse for #rejection_ident {
                fn into_response(self) -> ::axum::response::Response {
                    match self {
                        #(#arms)*
                    }
                }
            }
        }
    };

    let impl_display = if rejection_derive_opt_outs.derive_display() {
        let arms = fields
            .iter()
            .map(|field| {
                let variant_name = rejection_variant_name(field)?;
                Ok(quote! {
                    Self::#variant_name(inner) => inner.fmt(f),
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;

        Some(quote! {
            #[automatically_derived]
            impl ::std::fmt::Display for #rejection_ident {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    match self {
                        #(#arms)*
                    }
                }
            }
        })
    } else {
        None
    };

    let impl_error = if rejection_derive_opt_outs.derive_error() {
        let arms = fields
            .iter()
            .map(|field| {
                let variant_name = rejection_variant_name(field)?;
                Ok(quote! {
                    Self::#variant_name(inner) => Some(inner),
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;

        Some(quote! {
            #[automatically_derived]
            impl ::std::error::Error for #rejection_ident {
                fn source(&self) -> ::std::option::Option<&(dyn ::std::error::Error + 'static)> {
                    match self {
                        #(#arms)*
                    }
                }
            }
        })
    } else {
        None
    };

    let impl_debug = rejection_derive_opt_outs.derive_debug().then(|| {
        quote! { #[derive(Debug)] }
    });

    Ok(quote! {
        #impl_debug
        #vis enum #rejection_ident {
            #(#variants)*
        }

        #impl_into_response
        #impl_display
        #impl_error
    })
}

fn rejection_variant_name(field: &syn::Field) -> syn::Result<syn::Ident> {
    fn rejection_variant_name_for_type(out: &mut String, ty: &syn::Type) -> syn::Result<()> {
        if let syn::Type::Path(type_path) = ty {
            let segment = type_path
                .path
                .segments
                .last()
                .ok_or_else(|| syn::Error::new_spanned(ty, "Empty type path"))?;

            out.push_str(&segment.ident.to_string());

            match &segment.arguments {
                syn::PathArguments::AngleBracketed(args) => {
                    let ty = if args.args.len() == 1 {
                        args.args.last().unwrap()
                    } else if args.args.len() == 2 {
                        if segment.ident == "Result" {
                            args.args.first().unwrap()
                        } else {
                            return Err(syn::Error::new_spanned(
                                segment,
                                "Only `Result<T, E>` is supported with two generics type paramters",
                            ));
                        }
                    } else {
                        return Err(syn::Error::new_spanned(
                            &args.args,
                            "Expected exactly one or two type paramters",
                        ));
                    };

                    if let syn::GenericArgument::Type(ty) = ty {
                        rejection_variant_name_for_type(out, ty)
                    } else {
                        Err(syn::Error::new_spanned(ty, "Expected type path"))
                    }
                }
                syn::PathArguments::Parenthesized(args) => {
                    Err(syn::Error::new_spanned(args, "Unsupported"))
                }
                syn::PathArguments::None => Ok(()),
            }
        } else {
            Err(syn::Error::new_spanned(ty, "Expected type path"))
        }
    }

    if let Some(ident) = &field.ident {
        Ok(format_ident!("{}", ident.to_string().to_upper_camel_case()))
    } else {
        let mut out = String::new();
        rejection_variant_name_for_type(&mut out, &field.ty)?;

        let FromRequestFieldAttr { via } = parse_field_attrs(&field.attrs)?;
        if let Some((_, path)) = via {
            let via_ident = &path.segments.last().unwrap().ident;
            Ok(format_ident!("{}{}", via_ident, out))
        } else {
            Ok(format_ident!("{}", out))
        }
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

    let associated_rejection_type = if let Some(rejection) = &rejection {
        quote! { #rejection }
    } else {
        quote! {
            <#path<Self> as ::axum::extract::FromRequest<B, S>>::Rejection
        }
    };

    let rejection_bound = rejection.as_ref().map(|rejection| {
        if generic_ident.is_some() {
            quote! {
                #rejection: ::std::convert::From<<#path<T> as ::axum::extract::FromRequest<B, S>>::Rejection>,
            }
        } else {
            quote! {
                #rejection: ::std::convert::From<<#path<Self> as ::axum::extract::FromRequest<B, S>>::Rejection>,
            }
        }
    }).unwrap_or_default();

    let impl_generics = if generic_ident.is_some() {
        quote! { B, S, T }
    } else {
        quote! { B, S }
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
        impl<#impl_generics> ::axum::extract::FromRequest<B, S> for #ident #type_generics
        where
            #path<#via_type_generics>: ::axum::extract::FromRequest<B, S>,
            #rejection_bound
            B: ::std::marker::Send,
            S: ::std::marker::Send,
        {
            type Rejection = #associated_rejection_type;

            async fn from_request(
                req: &mut ::axum::extract::RequestParts<B, S>,
            ) -> ::std::result::Result<Self, Self::Rejection> {
                ::axum::extract::FromRequest::<B, S>::from_request(req)
                    .await
                    .map(|#path(value)| #value_to_self)
                    .map_err(::std::convert::From::from)
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

    let associated_rejection_type = if let Some(rejection) = rejection {
        quote! { #rejection }
    } else {
        quote! {
            <#path<Self> as ::axum::extract::FromRequest<B, S>>::Rejection
        }
    };

    let path_span = path.span();

    Ok(quote_spanned! {path_span=>
        #[::axum::async_trait]
        #[automatically_derived]
        impl<B, S> ::axum::extract::FromRequest<B, S> for #ident
        where
            B: ::axum::body::HttpBody + ::std::marker::Send + 'static,
            B::Data: ::std::marker::Send,
            B::Error: ::std::convert::Into<::axum::BoxError>,
            S: ::std::marker::Send,
        {
            type Rejection = #associated_rejection_type;

            async fn from_request(
                req: &mut ::axum::extract::RequestParts<B, S>,
            ) -> ::std::result::Result<Self, Self::Rejection> {
                ::axum::extract::FromRequest::<B, S>::from_request(req)
                    .await
                    .map(|#path(inner)| inner)
                    .map_err(::std::convert::From::from)
            }
        }
    })
}

#[test]
fn ui() {
    #[rustversion::stable]
    fn go() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/from_request/fail/*.rs");
        t.pass("tests/from_request/pass/*.rs");
    }

    #[rustversion::not(stable)]
    fn go() {}

    go();
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
