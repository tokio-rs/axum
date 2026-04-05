use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{parse::Parse, ItemStruct, LitStr, Token};

use crate::attr_parsing::{combine_attribute, parse_parenthesized_attribute, second, Combine};

pub(crate) fn expand(item_struct: &ItemStruct) -> syn::Result<TokenStream> {
    let ItemStruct {
        attrs,
        ident,
        generics,
        fields,
        ..
    } = item_struct;

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
            Ok(expand_named_fields(
                ident,
                &path,
                &segments,
                rejection.as_ref(),
            ))
        }
        syn::Fields::Unnamed(fields) => {
            let segments = parse_path(&path)?;
            expand_unnamed_fields(fields, ident, &path, &segments, rejection.as_ref())
        }
        syn::Fields::Unit => expand_unit_fields(ident, &path, rejection.as_ref()),
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
    path: &LitStr,
    segments: &[PathSegment],
    rejection: Option<&syn::Path>,
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
            #[allow(clippy::implicit_clone)]
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

    let rejection_assoc_type = rejection_assoc_type(rejection);
    let map_err_rejection = map_err_rejection(rejection);

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
                <::axum::extract::Path<#ident> as ::axum::extract::FromRequestParts<S>>
                    ::from_request_parts(parts, state)
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
    path: &LitStr,
    segments: &[PathSegment],
    rejection: Option<&syn::Path>,
) -> syn::Result<TokenStream> {
    let num_captures = captures_from_path(segments).len();
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

    let unnamed_captures = captures_from_path(segments);

    let destructure_self = unnamed_captures.iter().enumerate().map(|(idx, capture)| {
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
            #[allow(clippy::implicit_clone)]
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

    let rejection_assoc_type = rejection_assoc_type(rejection);
    let map_err_rejection = map_err_rejection(rejection);

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
    path: &LitStr,
    rejection: Option<&syn::Path>,
) -> syn::Result<TokenStream> {
    let has_captures = parse_path(path)?.iter().any(|segment| {
        segment
            .parts
            .iter()
            .any(|part| matches!(part, SegmentPart::Capture(_, _)))
    });

    if has_captures {
        return Err(syn::Error::new(
            path.span(),
            "Typed paths for unit structs cannot contain captures",
        ));
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

fn format_str_from_path(segments: &[PathSegment]) -> String {
    let mut path = String::new();

    for (idx, segment) in segments.iter().enumerate() {
        if idx > 0 {
            path.push('/');
        }

        for part in &segment.parts {
            match part {
                SegmentPart::Capture(capture, _) => {
                    path.push('{');
                    path.push_str(capture);
                    path.push('}');
                }
                SegmentPart::Static(static_part) => {
                    // Escape braces since this string is used as a `write!` format string.
                    for ch in static_part.chars() {
                        match ch {
                            '{' => path.push_str("{{"),
                            '}' => path.push_str("}}"),
                            _ => path.push(ch),
                        }
                    }
                }
            }
        }
    }

    path
}

fn captures_from_path(segments: &[PathSegment]) -> Vec<syn::Ident> {
    segments
        .iter()
        .flat_map(|segment| &segment.parts)
        .filter_map(|part| match part {
            SegmentPart::Capture(capture, span) => Some(format_ident!("{}", capture, span = *span)),
            SegmentPart::Static(_) => None,
        })
        .collect()
}

fn parse_path(path: &LitStr) -> syn::Result<Vec<PathSegment>> {
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
        .map(|segment| Ok(PathSegment::new(segment, path.span())))
        .collect()
}

struct PathSegment {
    parts: Vec<SegmentPart>,
}

impl PathSegment {
    fn new(segment: &str, span: Span) -> Self {
        let mut parts = Vec::new();
        let mut remaining = segment;

        while let Some(capture_start) = remaining.find('{') {
            let (before_capture, after_capture_start) = remaining.split_at(capture_start);
            if !before_capture.is_empty() {
                parts.push(SegmentPart::Static(before_capture.to_owned()));
            }

            let after_open = &after_capture_start[1..];
            if let Some(capture_end) = after_open.find('}') {
                let (capture, after_capture) = after_open.split_at(capture_end);
                let after_capture = &after_capture[1..];

                if !capture.is_empty() && !capture.contains(['{', '}']) {
                    let capture = capture.strip_prefix('*').unwrap_or(capture);
                    parts.push(SegmentPart::Capture(capture.to_owned(), span));
                    remaining = after_capture;
                    continue;
                }
            }

            parts.push(SegmentPart::Static("{".to_owned()));
            remaining = after_open;
        }

        if !remaining.is_empty() {
            parts.push(SegmentPart::Static(remaining.to_owned()));
        }

        Self { parts }
    }
}

enum SegmentPart {
    Capture(String, Span),
    Static(String),
}

fn path_rejection() -> TokenStream {
    quote! {
        <::axum::extract::Path<Self> as ::axum::extract::FromRequestParts<S>>::Rejection
    }
}

fn rejection_assoc_type(rejection: Option<&syn::Path>) -> TokenStream {
    match rejection {
        Some(rejection) => quote! { #rejection },
        None => path_rejection(),
    }
}

fn map_err_rejection(rejection: Option<&syn::Path>) -> TokenStream {
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
