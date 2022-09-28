use std::collections::HashMap;

use syn::{parse::Parse, punctuated::Punctuated, Token, Type};

use crate::attr_parsing::parse_assignment_attribute;

struct GenericArgSpecializationAttr {
    arg_name: syn::Ident,
    specialization_ty: syn::Type,
}

impl Parse for GenericArgSpecializationAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let arg_name = input.parse()?;
        input.parse::<syn::Token![=]>()?;
        let specialization_ty = input.parse()?;
        Ok(Self {
            arg_name,
            specialization_ty,
        })
    }
}

pub(crate) struct SpecializationsAttr {
    pub(crate) specializations: HashMap<syn::Ident, Vec<syn::Type>>,
}

impl Parse for SpecializationsAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let specialization_groups: Punctuated<
            Punctuated<GenericArgSpecializationAttr, Token![,]>,
            Token![;],
        > = Punctuated::parse_separated_nonempty_with(input, Punctuated::parse_separated_nonempty)?;
        let mut specializations: HashMap<syn::Ident, Vec<syn::Type>> = HashMap::new();
        for spec_group in specialization_groups {
            let arg_name = match spec_group.first() {
                Some(spec) => spec.arg_name.clone(),
                None => continue, // this should never happen due to parse_nonempty
            };
            if specializations.contains_key(&arg_name) {
                return Err(syn::Error::new(
                    arg_name.span(),
                    "Duplicate argument name specified. Each argument can only appear in one group. Groups are separated by semicolons. (e.g. `T = Foo, T = Bar; U = i64` is valid, but `T = Foo, T = Bar; T = i64` is not).",
                ));
            }
            let mut group_specs = Vec::new();
            for spec in spec_group {
                if spec.arg_name != arg_name {
                    return Err(syn::Error::new(
                        spec.arg_name.span(),
                        "All argument names in a group must match. Groups should be separated by `;`. (e.g. `T = Foo, T = Bar; U = i64` is valid, but `T = Foo, U = Bar` is not).",
                    ));
                }
                group_specs.push(spec.specialization_ty);
            }
            specializations.insert(arg_name, group_specs);
        }
        Ok(Self { specializations })
    }
}

mod kw {
    syn::custom_keyword!(body);
    syn::custom_keyword!(state);
    syn::custom_keyword!(with);
}

pub(crate) struct Attrs {
    pub(crate) body_ty: Option<(kw::body, Type)>,
    pub(crate) state_ty: Option<(kw::state, Type)>,
    pub(crate) with_tys: Option<SpecializationsAttr>,
}

impl Parse for Attrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut body_ty = None;
        let mut state_ty = None;
        let mut with_tys = None;

        while !input.is_empty() {
            let lh = input.lookahead1();
            if lh.peek(kw::body) {
                parse_assignment_attribute(input, &mut body_ty)?;
            } else if lh.peek(kw::state) {
                parse_assignment_attribute(input, &mut state_ty)?;
            } else if lh.peek(kw::with) {
                let _: kw::with = input.parse()?;
                let content;
                syn::parenthesized!(content in input);
                with_tys = Some(content.parse()?);
            } else {
                return Err(lh.error());
            }

            let _ = input.parse::<Token![,]>();
        }

        Ok(Self {
            body_ty,
            state_ty,
            with_tys,
        })
    }
}
