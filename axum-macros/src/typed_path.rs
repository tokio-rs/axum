use std::collections::HashSet;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{
    parse::Parse, parse_quote, punctuated::Punctuated, GenericParam, Generics, ItemStruct, LitStr,
    Token, WhereClause, WherePredicate,
};

use crate::attr_parsing::{combine_attribute, parse_parenthesized_attribute, second, Combine};

pub(crate) fn expand(item_struct: ItemStruct) -> syn::Result<TokenStream> {
    let ItemStruct {
        attrs,
        ident,
        generics,
        fields,
        ..
    } = item_struct;

    let Attrs { path, rejection } = crate::attr_parsing::parse_attrs("typed_path", &attrs)?;

    let path = path.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "Missing path: `#[typed_path(\"/foo/bar\")]`",
        )
    })?;

    let rejection = rejection.map(second);

    match fields {
        syn::Fields::Named(fields) => {
            let segments = parse_path(&path)?;
            Ok(expand_named_fields(
                fields, ident, path, &segments, rejection, generics,
            ))
        }
        syn::Fields::Unnamed(fields) => {
            let segments = parse_path(&path)?;
            expand_unnamed_fields(fields, ident, path, &segments, rejection, generics)
        }
        syn::Fields::Unit => expand_unit_fields(ident, path, rejection, generics),
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
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
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
    fields: syn::FieldsNamed,
    ident: syn::Ident,
    path: LitStr,
    segments: &[Segment],
    rejection: Option<syn::Path>,
    generics: Generics,
) -> TokenStream {
    let format_str = format_str_from_path(segments);
    let captures = captures_from_path(segments);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let field_types = fields
        .named
        .iter()
        .map(|field| &field.ty)
        .cloned()
        .collect::<HashSet<_>>();
    let generic_types = extract_generic_types(&generics);

    let path_where_clause = add_where_bounds_for_types(
        where_clause,
        // only generate where clause bounds for types that are both in the generics
        // and used for struct fields directly
        field_types.intersection(&generic_types),
        |ty| parse_quote! { #ty: ::std::fmt::Display },
    );

    let typed_path_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl #impl_generics ::axum_extra::routing::TypedPath for #ident #ty_generics #path_where_clause {
            const PATH: &'static str = #path;
        }
    };

    let display_where_clause = add_where_bounds_for_types(
        where_clause,
        // only generate where clause bounds for types that are both in the generics
        // and used for struct fields directly
        field_types.intersection(&generic_types),
        |ty| parse_quote! { #ty: ::std::fmt::Display },
    );

    let display_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl #impl_generics ::std::fmt::Display for #ident #ty_generics #display_where_clause {
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

    let mut parts_where_clause = add_where_bounds_for_types(
        where_clause,
        // only generate where clause bounds for types that are both in the generics
        // and used for struct fields directly
        field_types.intersection(&generic_types),
        |ty| parse_quote! { for<'de> #ty: Send + Sync + ::serde::Deserialize<'de> },
    )
    .unwrap_or_else(empty_where_clause);

    parts_where_clause.predicates.push(parse_quote!(
        __Derived_S: Send + Sync
    ));

    let mut parts_generics = generics.clone();

    parts_generics.params.push(parse_quote! {__Derived_S});
    let (impl_generics, _, _) = parts_generics.split_for_impl();

    let from_request_impl = quote! {
        #[::axum::async_trait]
        #[automatically_derived]
        impl #impl_generics ::axum::extract::FromRequestParts<__Derived_S> for #ident #ty_generics #parts_where_clause {
            type Rejection = #rejection_assoc_type;

            async fn from_request_parts(
                parts: &mut ::axum::http::request::Parts,
                state: &__Derived_S,
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
    fields: syn::FieldsUnnamed,
    ident: syn::Ident,
    path: LitStr,
    segments: &[Segment],
    rejection: Option<syn::Path>,
    generics: Generics,
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

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let field_types = fields
        .unnamed
        .iter()
        .map(|field| &field.ty)
        .cloned()
        .collect::<HashSet<_>>();
    let generic_types = extract_generic_types(&generics);

    let path_where_clause = add_where_bounds_for_types(
        where_clause,
        // only generate where clause bounds for types that are both in the generics
        // and used for struct fields directly
        field_types.intersection(&generic_types),
        |ty| parse_quote! { #ty: ::std::fmt::Display },
    );

    let format_str = format_str_from_path(segments);
    let captures = captures_from_path(segments);

    let typed_path_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl #impl_generics ::axum_extra::routing::TypedPath for #ident #ty_generics #path_where_clause {
            const PATH: &'static str = #path;
        }
    };

    let display_where_clause = add_where_bounds_for_types(
        where_clause,
        // only generate where clause bounds for types that are both in the generics
        // and used for struct fields directly
        field_types.intersection(&generic_types),
        |ty| parse_quote! {#ty: ::std::fmt::Display},
    );

    let display_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl #impl_generics ::std::fmt::Display for #ident #ty_generics #display_where_clause {
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

    let mut parts_where_clause = add_where_bounds_for_types(
        where_clause,
        // only generate where clause bounds for types that are both in the generics
        // and used for struct fields directly
        field_types.intersection(&generic_types),
        |ty| parse_quote! {for<'de> #ty: Send + Sync + ::serde::Deserialize<'de> },
    )
    .unwrap_or_else(empty_where_clause);

    parts_where_clause.predicates.push(parse_quote!(
        __Derived_S: Send + Sync
    ));

    let mut parts_generics = generics.clone();

    parts_generics.params.push(parse_quote! {__Derived_S});
    let (impl_generics, _, _) = parts_generics.split_for_impl();

    let from_request_impl = quote! {
        #[::axum::async_trait]
        #[automatically_derived]
        impl #impl_generics ::axum::extract::FromRequestParts<__Derived_S> for #ident #ty_generics #where_clause {
            type Rejection = #rejection_assoc_type;

            async fn from_request_parts(
                parts: &mut ::axum::http::request::Parts,
                state: &__Derived_S,
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
    ident: syn::Ident,
    path: LitStr,
    rejection: Option<syn::Path>,
    generics: Generics,
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

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let typed_path_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl #impl_generics ::axum_extra::routing::TypedPath for #ident #ty_generics #where_clause {
            const PATH: &'static str = #path;
        }
    };

    let display_impl = quote_spanned! {path.span()=>
        #[automatically_derived]
        impl #impl_generics ::std::fmt::Display for #ident #ty_generics #where_clause {
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

    let mut parts_where_clause = where_clause.cloned().unwrap_or_else(empty_where_clause);

    parts_where_clause.predicates.push(parse_quote!(
        __Derived_S: Send + Sync
    ));

    let mut parts_generics = generics.clone();

    parts_generics.params.push(parse_quote! {__Derived_S});
    let (impl_generics, _, _) = parts_generics.split_for_impl();

    let from_request_impl = quote! {
        #[::axum::async_trait]
        #[automatically_derived]
        impl #impl_generics ::axum::extract::FromRequestParts<__Derived_S> for #ident #ty_generics #parts_where_clause {
            type Rejection = #rejection_assoc_type;

            async fn from_request_parts(
                parts: &mut ::axum::http::request::Parts,
                _state: &__Derived_S,
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
                .strip_prefix(':')
                .or_else(|| segment.strip_prefix('*'))
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
        <::axum::extract::Path<Self> as ::axum::extract::FromRequestParts<__Derived_S>>::Rejection
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

fn extract_generic_types(generics: &Generics) -> HashSet<syn::Type> {
    generics
        .params
        .iter()
        .filter_map(|g| match g {
            GenericParam::Type(t) => syn::parse2(t.ident.to_token_stream()).ok(),
            _ => None,
        })
        .collect()
}

fn empty_where_clause() -> WhereClause {
    WhereClause {
        where_token: Token![where](Span::mixed_site()),
        predicates: Punctuated::new(),
    }
}

fn add_where_bounds_for_types<'a, 'b>(
    where_clause: Option<&'a WhereClause>,
    types: impl IntoIterator<Item = &'b syn::Type>,
    bound: impl Fn(&'b syn::Type) -> WherePredicate,
) -> Option<WhereClause> {
    let mut peekable_types = types.into_iter().peekable();

    peekable_types.peek()?;

    let mut where_clause = where_clause.cloned().unwrap_or_else(empty_where_clause);

    for ty in peekable_types {
        where_clause.predicates.push(bound(ty));
    }

    Some(where_clause)
}

#[test]
fn ui() {
    crate::run_ui_tests("typed_path");
}
