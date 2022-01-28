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

#[derive(Default)]
pub(crate) struct FromRequestContainerAttr {
    pub(crate) via: Option<(kw::via, syn::Path)>,
    pub(crate) rejection_derive: Option<(kw::rejection_derive, RejectionDeriveOptOuts)>,
}

pub(crate) mod kw {
    syn::custom_keyword!(via);
    syn::custom_keyword!(rejection_derive);
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
    let attrs = parse_attrs(attrs)?;

    let mut out = FromRequestContainerAttr::default();

    for from_request_attr in attrs {
        match from_request_attr {
            ContainerAttr::Via { via, path } => {
                if out.via.is_some() {
                    return Err(double_attr_error("via", via));
                } else {
                    out.via = Some((via, path));
                }
            }
            ContainerAttr::RejectionDerive {
                rejection_derive,
                opt_outs,
            } => {
                if out.rejection_derive.is_some() {
                    return Err(double_attr_error("rejection_derive", rejection_derive));
                } else {
                    out.rejection_derive = Some((rejection_derive, opt_outs));
                }
            }
        }
    }

    Ok(out)
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
    RejectionDerive {
        rejection_derive: kw::rejection_derive,
        opt_outs: RejectionDeriveOptOuts,
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
        } else if lh.peek(kw::rejection_derive) {
            let rejection_derive = input.parse::<kw::rejection_derive>()?;
            let content;
            syn::parenthesized!(content in input);
            content.parse().map(|opt_outs| Self::RejectionDerive {
                rejection_derive,
                opt_outs,
            })
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

#[derive(Debug, Default)]
pub(crate) struct RejectionDeriveOptOuts {
    debug: Option<kw::Debug>,
    display: Option<kw::Display>,
    error: Option<kw::Error>,
}

impl RejectionDeriveOptOuts {
    pub(crate) fn derive_debug(&self) -> bool {
        self.debug.is_none()
    }

    pub(crate) fn derive_display(&self) -> bool {
        self.display.is_none()
    }

    pub(crate) fn derive_error(&self) -> bool {
        self.error.is_none()
    }
}

impl Parse for RejectionDeriveOptOuts {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut debug = None;
        let mut display = None;
        let mut error = None;

        while !input.is_empty() {
            input.parse::<Token![!]>()?;

            let lh = input.lookahead1();
            if lh.peek(kw::Debug) {
                debug = Some(input.parse()?);
            } else if lh.peek(kw::Display) {
                display = Some(input.parse()?);
            } else if lh.peek(kw::Error) {
                error = Some(input.parse()?);
            } else {
                return Err(lh.error());
            }

            input.parse::<Token![,]>().ok();
        }

        Ok(Self {
            debug,
            display,
            error,
        })
    }
}
