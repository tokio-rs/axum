use std::collections::HashMap;

use syn::{parse::Parse, punctuated::Punctuated, Token, Type};

pub(crate) struct GenericArgSpecializationAttr {
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

pub(crate) struct Attrs {
    body_ty: Type,
    with_tys: Option<Punctuated<GenericArgSpecializationAttr, Token![,]>>,
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
                with_tys = Some(content.parse_terminated(GenericArgSpecializationAttr::parse)?);
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

    pub(crate) fn compute_specializations(&self) -> HashMap<syn::Ident, Vec<syn::Type>> {
        let mut grouped: HashMap<syn::Ident, Vec<syn::Type>> = HashMap::new();
        if let Some(with_tys) = &self.with_tys {
            for GenericArgSpecializationAttr {
                arg_name,
                specialization_ty,
            } in with_tys.iter()
            {
                let specialization_ty = specialization_ty.clone();
                if let Some(specializations) = grouped.get_mut(arg_name) {
                    specializations.push(specialization_ty);
                } else {
                    grouped.insert(arg_name.clone(), vec![specialization_ty]);
                }
            }
        }
        grouped
    }
}
