use std::collections::HashMap;

use syn::{parse::Parse, punctuated::Punctuated, Token, Type};

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

struct SpecializationsAttr {
    specializations: HashMap<syn::Ident, Vec<syn::Type>>,
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

pub(crate) struct Attrs {
    body_ty: Type,
    with_tys: Option<SpecializationsAttr>,
}

impl Parse for Attrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut body_ty = None;
        let mut with_tys = None;

        while !input.is_empty() {
            let ident = input.parse::<syn::Ident>()?;
            if ident == "body" {
                input.parse::<Token![=]>()?;
                body_ty = Some(input.parse()?);
            } else if ident == "with" {
                let content;
                syn::parenthesized!(content in input);
                with_tys = Some(content.parse()?);
            } else {
                return Err(syn::Error::new_spanned(ident, "unknown argument"));
            }

            let _ = input.parse::<Token![,]>();
        }

        let body_ty = body_ty.unwrap_or_else(|| syn::parse_quote!(axum::body::Body));

        Ok(Self { body_ty, with_tys })
    }
}

impl Attrs {
    pub(crate) fn body_ty(&self) -> &Type {
        &self.body_ty
    }
    pub(crate) fn specializations(&self) -> Option<&HashMap<syn::Ident, Vec<syn::Type>>> {
        self.with_tys.as_ref().map(|f| &f.specializations)
    }
}
