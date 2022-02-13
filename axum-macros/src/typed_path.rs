use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{ItemStruct, LitStr};

pub(crate) fn expand(item_struct: ItemStruct) -> syn::Result<TokenStream> {
    let ItemStruct {
        attrs,
        vis: _,
        struct_token: _,
        ident,
        generics,
        fields,
        semi_token: _,
    } = &item_struct;

    if !generics.params.is_empty() || generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            generics,
            "`#[derive(TypePath)]` doesn't support generics",
        ));
    }

    let Attrs { path } = parse_attrs(attrs)?;

    match fields {
        syn::Fields::Named(_) => {
            let segments = parse_path(&path);
            Ok(expand_named_fields(ident, path, &segments))
        }
        syn::Fields::Unnamed(fields) => {
            let segments = parse_path(&path);
            expand_unnamed_fields(fields, ident, path, &segments)
        }
        syn::Fields::Unit => Ok(expand_unit_fields(ident, path)),
    }
}

#[derive(Debug)]
struct Attrs {
    path: String,
}

fn parse_attrs(attrs: &[syn::Attribute]) -> syn::Result<Attrs> {
    let mut path = None::<String>;

    for attr in attrs {
        if attr.path.is_ident("typed_path") {
            path = Some(attr.parse_args::<LitStr>()?.value());
        }
    }

    Ok(Attrs {
        path: path.ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                "missing `#[typed_path(\"...\")]` attribute",
            )
        })?,
    })
}

fn expand_named_fields(ident: &syn::Ident, path: String, segments: &[Segment]) -> TokenStream {
    let format_str = format_str_from_path(segments);
    let captures = captures_from_path(segments);

    quote! {
        #[automatically_derived]
        impl ::axum_extra::routing::TypedPath for #ident {
            const PATH: &'static str = #path;

            fn path(&self) -> ::std::borrow::Cow<'static, str> {
                let Self { #(#captures,)* } = self;
                format!(#format_str, #(#captures = #captures,)*).into()
            }
        }
    }
}

fn expand_unnamed_fields(
    fields: &syn::FieldsUnnamed,
    ident: &syn::Ident,
    path: String,
    segments: &[Segment],
) -> syn::Result<TokenStream> {
    let num_captures = segments
        .iter()
        .filter(|segment| match segment {
            Segment::Capture(_) => true,
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
            Segment::Capture(capture) => Some(capture),
            Segment::Static(_) => None,
        })
        .enumerate()
        .map(|(idx, capture)| {
            let idx = syn::Index {
                index: idx as _,
                span: Span::call_site(),
            };
            let capture = format_ident!("{}", capture);
            quote! {
                #idx: #capture,
            }
        });

    let format_str = format_str_from_path(segments);
    let captures = captures_from_path(segments);

    Ok(quote! {
        #[automatically_derived]
        impl ::axum_extra::routing::TypedPath for #ident {
            const PATH: &'static str = #path;

            fn path(&self) -> ::std::borrow::Cow<'static, str> {
                let Self { #(#destructure_self)* } = self;
                format!(#format_str, #(#captures = #captures,)*).into()
            }
        }
    })
}

fn simple_pluralize(count: usize, word: &str) -> String {
    if count == 1 {
        format!("{} {}", count, word)
    } else {
        format!("{} {}s", count, word)
    }
}

fn expand_unit_fields(ident: &syn::Ident, path: String) -> TokenStream {
    quote! {
        #[automatically_derived]
        impl ::axum_extra::routing::TypedPath for #ident {
            const PATH: &'static str = #path;

            fn path(&self) -> ::std::borrow::Cow<'static, str> {
                #path.into()
            }
        }
    }
}

fn format_str_from_path(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|segment| match segment {
            Segment::Capture(capture) => format!("{{{}}}", capture),
            Segment::Static(segment) => segment.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn captures_from_path(segments: &[Segment]) -> Vec<syn::Ident> {
    segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Capture(capture) => Some(format_ident!("{}", capture)),
            Segment::Static(_) => None,
        })
        .collect::<Vec<_>>()
}

fn parse_path(path: &str) -> Vec<Segment> {
    path.split('/')
        .map(|segment| {
            if let Some(capture) = segment.strip_prefix(':') {
                Segment::Capture(capture.to_owned())
            } else {
                Segment::Static(segment.to_owned())
            }
        })
        .collect()
}

enum Segment {
    Capture(String),
    Static(String),
}
