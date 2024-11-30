use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{parse::Parse, ItemStruct, LitStr, Token};

use crate::attr_parsing::{combine_attribute, parse_parenthesized_attribute, second, Combine};

pub(crate) fn expand(item_struct: ItemStruct) -> syn::Result<TokenStream> {
    let ItemStruct {
        attrs,
        ident,
        generics,
        fields,
        ..
    } = &item_struct;

    if !generics.params.is_empty() || generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            generics,
            "`#[derive(TypedPath)]` doesn't support generics",
        ));
    }

    let Attrs { path, rejection } = crate::attr_parsing::parse_attrs("typed_path", attrs)?;

    let path = path.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "Missing path: `#[typed_path(\"/foo/bar\")]`",
        )
    })?;

    let rejection = rejection.map(second);

    match fields {
        syn::Fields::Named(_) => {
            let segments = parse_path(&path)?;
            Ok(expand_named_fields(ident, path, &segments, rejection))
        }
        syn::Fields::Unnamed(fields) => {
            let segments = parse_path(&path)?;
            expand_unnamed_fields(fields, ident, path, &segments, rejection)
        }
        syn::Fields::Unit => expand_unit_fields(ident, path, rejection),
    }
}

mod kw {
    syn::custom_keyword!(rejection);
}

#[derive(Default)]
struct Attrs {
    path: Option<LitStr>,
    rejection: Option<(kw::rejection, syn::Path)>,
}

impl Parse for Attrs {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let mut path = None;
        let mut rejection = None;

        while !input.is_empty() {
            let lh = input.lookahead1();
            if lh.peek(LitStr) {
                path = Some(input.parse()?);
            } else if lh.peek(kw::rejection) {
                parse_parenthesized_attribute(input, &mut rejection)?;
            } else {
                return Err(lh.error());
            }

            let _ = input.parse::<Token![,]>();
        }

        Ok(Self { path, rejection })
    }
}

impl Combine for Attrs {
    fn combine(mut self, other: Self) -> syn::Result<Self> {
        let Self { path, rejection } = other;
        if let Some(path) = path {
            if self.path.is_some() {
                return Err(syn::Error::new_spanned(
                    path,
                    "path specified more than once",
                ));
            }
            self.path = Some(path);
        }
        combine_attribute(&mut self.rejection, rejection)?;
        Ok(self)
    }
}

fn expand_named_fields(
    ident: &syn::Ident,
    path: LitStr,
    segments: &[Segment],
    rejection: Option<syn::Path>,
) -> TokenStream {
    let format_str = format_str_from_path(segments);
    let captures = captures_from_path(segments);

    let typed_path_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl ::axum_extra::routing::TypedPath for #ident {
            const PATH: &'static str = #path;
        }
    };

    let display_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl ::std::fmt::Display for #ident {
            #[allow(clippy::unnecessary_to_owned)]
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let Self { #(#captures,)* } = self;
                write!(
                    f,
                    #format_str,
                    #(
                        #captures = ::axum_extra::__private::utf8_percent_encode(
                            &#captures.to_string(),
                            ::axum_extra::__private::PATH_SEGMENT,
                        )
                    ),*
                )
            }
        }
    };

    let rejection_assoc_type = rejection_assoc_type(&rejection);
    let map_err_rejection = map_err_rejection(&rejection);

    let from_request_impl = quote! {
        #[automatically_derived]
        impl<S> ::axum::extract::FromRequestParts<S> for #ident
        where
            S: Send + Sync,
        {
            type Rejection = #rejection_assoc_type;

            async fn from_request_parts(
                parts: &mut ::axum::http::request::Parts,
                state: &S,
            ) -> ::std::result::Result<Self, Self::Rejection> {
                ::axum::extract::Path::from_request_parts(parts, state)
                    .await
                    .map(|path| path.0)
                    #map_err_rejection
            }
        }
    };

    quote! {
        #typed_path_impl
        #display_impl
        #from_request_impl
    }
}

fn expand_unnamed_fields(
    fields: &syn::FieldsUnnamed,
    ident: &syn::Ident,
    path: LitStr,
    segments: &[Segment],
    rejection: Option<syn::Path>,
) -> syn::Result<TokenStream> {
    let num_captures = segments
        .iter()
        .filter(|segment| match segment {
            Segment::Capture(_, _) => true,
            Segment::Static(_) => false,
        })
        .count();
    let num_fields = fields.unnamed.len();
    if num_fields != num_captures {
        return Err(syn::Error::new_spanned(
            fields,
            format!(
                "Mismatch in number of captures and fields. Path has {} but struct has {}",
                simple_pluralize(num_captures, "capture"),
                simple_pluralize(num_fields, "field"),
            ),
        ));
    }

    let destructure_self = segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Capture(capture, _) => Some(capture),
            Segment::Static(_) => None,
        })
        .enumerate()
        .map(|(idx, capture)| {
            let idx = syn::Index {
                index: idx as _,
                span: Span::call_site(),
            };
            let capture = format_ident!("{}", capture, span = path.span());
            quote_spanned! {path.span()=>
                #idx: #capture,
            }
        });

    let format_str = format_str_from_path(segments);
    let captures = captures_from_path(segments);

    let typed_path_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl ::axum_extra::routing::TypedPath for #ident {
            const PATH: &'static str = #path;
        }
    };

    let display_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl ::std::fmt::Display for #ident {
            #[allow(clippy::unnecessary_to_owned)]
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let Self { #(#destructure_self)* } = self;
                write!(
                    f,
                    #format_str,
                    #(
                        #captures = ::axum_extra::__private::utf8_percent_encode(
                            &#captures.to_string(),
                            ::axum_extra::__private::PATH_SEGMENT,
                        )
                    ),*
                )
            }
        }
    };

    let rejection_assoc_type = rejection_assoc_type(&rejection);
    let map_err_rejection = map_err_rejection(&rejection);

    let from_request_impl = quote! {
        #[automatically_derived]
        impl<S> ::axum::extract::FromRequestParts<S> for #ident
        where
            S: Send + Sync,
        {
            type Rejection = #rejection_assoc_type;

            async fn from_request_parts(
                parts: &mut ::axum::http::request::Parts,
                state: &S,
            ) -> ::std::result::Result<Self, Self::Rejection> {
                ::axum::extract::Path::from_request_parts(parts, state)
                    .await
                    .map(|path| path.0)
                    #map_err_rejection
            }
        }
    };

    Ok(quote! {
        #typed_path_impl
        #display_impl
        #from_request_impl
    })
}

fn simple_pluralize(count: usize, word: &str) -> String {
    if count == 1 {
        format!("{count} {word}")
    } else {
        format!("{count} {word}s")
    }
}

fn expand_unit_fields(
    ident: &syn::Ident,
    path: LitStr,
    rejection: Option<syn::Path>,
) -> syn::Result<TokenStream> {
    for segment in parse_path(&path)? {
        match segment {
            Segment::Capture(_, span) => {
                return Err(syn::Error::new(
                    span,
                    "Typed paths for unit structs cannot contain captures",
                ));
            }
            Segment::Static(_) => {}
        }
    }

    let typed_path_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl ::axum_extra::routing::TypedPath for #ident {
            const PATH: &'static str = #path;
        }
    };

    let display_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl ::std::fmt::Display for #ident {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, #path)
            }
        }
    };

    let rejection_assoc_type = if let Some(rejection) = &rejection {
        quote! { #rejection }
    } else {
        quote! { ::axum::http::StatusCode }
    };
    let create_rejection = if let Some(rejection) = &rejection {
        quote! {
            Err(<#rejection as ::std::default::Default>::default())
        }
    } else {
        quote! {
            Err(::axum::http::StatusCode::NOT_FOUND)
        }
    };

    let from_request_impl = quote! {
        #[automatically_derived]
        impl<S> ::axum::extract::FromRequestParts<S> for #ident
        where
            S: Send + Sync,
        {
            type Rejection = #rejection_assoc_type;

            async fn from_request_parts(
                parts: &mut ::axum::http::request::Parts,
                _state: &S,
            ) -> ::std::result::Result<Self, Self::Rejection> {
                if parts.uri.path() == <Self as ::axum_extra::routing::TypedPath>::PATH {
                    Ok(Self)
                } else {
                    #create_rejection
                }
            }
        }
    };

    Ok(quote! {
        #typed_path_impl
        #display_impl
        #from_request_impl
    })
}

fn format_str_from_path(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|segment| match segment {
            Segment::Capture(capture, _) => format!("{{{capture}}}"),
            Segment::Static(segment) => segment.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn captures_from_path(segments: &[Segment]) -> Vec<syn::Ident> {
    segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Capture(capture, span) => Some(format_ident!("{}", capture, span = *span)),
            Segment::Static(_) => None,
        })
        .collect::<Vec<_>>()
}

fn parse_path(path: &LitStr) -> syn::Result<Vec<Segment>> {
    let value = path.value();
    if value.is_empty() {
        return Err(syn::Error::new_spanned(
            path,
            "paths must start with a `/`. Use \"/\" for root routes",
        ));
    } else if !path.value().starts_with('/') {
        return Err(syn::Error::new_spanned(path, "paths must start with a `/`"));
    }

    path.value()
        .split('/')
        .map(|segment| {
            if let Some(capture) = segment
                .strip_prefix('{')
                .and_then(|segment| segment.strip_suffix('}'))
                .and_then(|segment| {
                    (!segment.starts_with('{') && !segment.ends_with('}')).then_some(segment)
                })
                .map(|capture| capture.strip_prefix('*').unwrap_or(capture))
            {
                Ok(Segment::Capture(capture.to_owned(), path.span()))
            } else {
                Ok(Segment::Static(segment.to_owned()))
            }
        })
        .collect()
}

enum Segment {
    Capture(String, Span),
    Static(String),
}

fn path_rejection() -> TokenStream {
    quote! {
        <::axum::extract::Path<Self> as ::axum::extract::FromRequestParts<S>>::Rejection
    }
}

fn rejection_assoc_type(rejection: &Option<syn::Path>) -> TokenStream {
    match rejection {
        Some(rejection) => quote! { #rejection },
        None => path_rejection(),
    }
}

fn map_err_rejection(rejection: &Option<syn::Path>) -> TokenStream {
    rejection
        .as_ref()
        .map(|rejection| {
            let path_rejection = path_rejection();
            quote! {
                .map_err(|rejection| {
                    <#rejection as ::std::convert::From<#path_rejection>>::from(rejection)
                })
            }
        })
        .unwrap_or_default()
}

#[test]
fn ui() {
    crate::run_ui_tests("typed_path");
}
