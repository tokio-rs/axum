use crate::attr_parsing::{combine_attribute, parse_parenthesized_attribute, Combine};
use syn::{
    parse::{Parse, ParseStream},
    Token,
};

pub(crate) mod kw {
    syn::custom_keyword!(via);
    syn::custom_keyword!(rejection);
    syn::custom_keyword!(state);
}

#[derive(Default)]
pub(super) struct FromRequestContainerAttrs {
    pub(super) via: Option<(kw::via, syn::Path)>,
    pub(super) rejection: Option<(kw::rejection, syn::Path)>,
    pub(super) state: Option<(kw::state, syn::Type)>,
}

impl Parse for FromRequestContainerAttrs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut via = None;
        let mut rejection = None;
        let mut state = None;

        while !input.is_empty() {
            let lh = input.lookahead1();
            if lh.peek(kw::via) {
                parse_parenthesized_attribute(input, &mut via)?;
            } else if lh.peek(kw::rejection) {
                parse_parenthesized_attribute(input, &mut rejection)?;
            } else if lh.peek(kw::state) {
                parse_parenthesized_attribute(input, &mut state)?;
            } else {
                return Err(lh.error());
            }

            let _ = input.parse::<Token![,]>();
        }

        Ok(Self {
            via,
            rejection,
            state,
        })
    }
}

impl Combine for FromRequestContainerAttrs {
    fn combine(mut self, other: Self) -> syn::Result<Self> {
        let Self {
            via,
            rejection,
            state,
        } = other;
        combine_attribute(&mut self.via, via)?;
        combine_attribute(&mut self.rejection, rejection)?;
        combine_attribute(&mut self.state, state)?;
        Ok(self)
    }
}

#[derive(Default)]
pub(super) struct FromRequestFieldAttrs {
    pub(super) via: Option<(kw::via, syn::Path)>,
}

impl Parse for FromRequestFieldAttrs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut via = None;

        while !input.is_empty() {
            let lh = input.lookahead1();
            if lh.peek(kw::via) {
                parse_parenthesized_attribute(input, &mut via)?;
            } else {
                return Err(lh.error());
            }

            let _ = input.parse::<Token![,]>();
        }

        Ok(Self { via })
    }
}

impl Combine for FromRequestFieldAttrs {
    fn combine(mut self, other: Self) -> syn::Result<Self> {
        let Self { via } = other;
        combine_attribute(&mut self.via, via)?;
        Ok(self)
    }
}
