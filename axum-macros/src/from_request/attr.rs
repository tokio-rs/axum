use quote::ToTokens;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Token,
};

#[derive(Default)]
pub(crate) struct FromRequestFieldAttr {
    pub(crate) via: Option<(kw::via, syn::Path)>,
}

pub(crate) enum FromRequestContainerAttr {
    Via {
        path: syn::Path,
        rejection: Option<syn::Path>,
    },
    Rejection(syn::Path),
    None,
}

pub(crate) mod kw {
    syn::custom_keyword!(via);
    syn::custom_keyword!(rejection);
    syn::custom_keyword!(Display);
    syn::custom_keyword!(Debug);
    syn::custom_keyword!(Error);
}

pub(crate) fn parse_field_attrs(attrs: &[syn::Attribute]) -> syn::Result<FromRequestFieldAttr> {
    let attrs = parse_attrs(attrs)?;

    let mut out = FromRequestFieldAttr::default();

    for from_request_attr in attrs {
        match from_request_attr {
            FieldAttr::Via { via, path } => {
                if out.via.is_some() {
                    return Err(double_attr_error("via", via));
                } else {
                    out.via = Some((via, path));
                }
            }
        }
    }

    Ok(out)
}

pub(crate) fn parse_container_attrs(
    attrs: &[syn::Attribute],
) -> syn::Result<FromRequestContainerAttr> {
    let attrs = parse_attrs::<ContainerAttr>(attrs)?;

    let mut out_via = None;
    let mut out_rejection = None;

    // we track the index of the attribute to know which comes last
    // used to give more accurate error messages
    for (idx, from_request_attr) in attrs.into_iter().enumerate() {
        match from_request_attr {
            ContainerAttr::Via { via, path } => {
                if out_via.is_some() {
                    return Err(double_attr_error("via", via));
                } else {
                    out_via = Some((idx, via, path));
                }
            }
            ContainerAttr::Rejection { rejection, path } => {
                if out_rejection.is_some() {
                    return Err(double_attr_error("rejection", rejection));
                } else {
                    out_rejection = Some((idx, rejection, path));
                }
            }
        }
    }

    match (out_via, out_rejection) {
        (Some((_, _, path)), None) => Ok(FromRequestContainerAttr::Via {
            path,
            rejection: None,
        }),

        (Some((_, _, path)), Some((_, _, rejection))) => Ok(FromRequestContainerAttr::Via {
            path,
            rejection: Some(rejection),
        }),

        (None, Some((_, _, rejection))) => Ok(FromRequestContainerAttr::Rejection(rejection)),

        (None, None) => Ok(FromRequestContainerAttr::None),
    }
}

pub(crate) fn parse_attrs<T>(attrs: &[syn::Attribute]) -> syn::Result<Punctuated<T, Token![,]>>
where
    T: Parse,
{
    let attrs = attrs
        .iter()
        .filter(|attr| attr.path.is_ident("from_request"))
        .map(|attr| attr.parse_args_with(Punctuated::<T, Token![,]>::parse_terminated))
        .collect::<syn::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Punctuated<T, Token![,]>>();
    Ok(attrs)
}

fn double_attr_error<T>(ident: &str, spanned: T) -> syn::Error
where
    T: ToTokens,
{
    syn::Error::new_spanned(spanned, format!("`{}` specified more than once", ident))
}

enum ContainerAttr {
    Via {
        via: kw::via,
        path: syn::Path,
    },
    Rejection {
        rejection: kw::rejection,
        path: syn::Path,
    },
}

impl Parse for ContainerAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lh = input.lookahead1();
        if lh.peek(kw::via) {
            let via = input.parse::<kw::via>()?;
            let content;
            syn::parenthesized!(content in input);
            content.parse().map(|path| Self::Via { via, path })
        } else if lh.peek(kw::rejection) {
            let rejection = input.parse::<kw::rejection>()?;
            let content;
            syn::parenthesized!(content in input);
            content
                .parse()
                .map(|path| Self::Rejection { rejection, path })
        } else {
            Err(lh.error())
        }
    }
}

enum FieldAttr {
    Via { via: kw::via, path: syn::Path },
}

impl Parse for FieldAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lh = input.lookahead1();
        if lh.peek(kw::via) {
            let via = input.parse::<kw::via>()?;
            let content;
            syn::parenthesized!(content in input);
            content.parse().map(|path| Self::Via { via, path })
        } else {
            Err(lh.error())
        }
    }
}
