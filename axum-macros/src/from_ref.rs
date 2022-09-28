use crate::attr_parsing::{parse_attrs, Combine};
use proc_macro2::{Ident, TokenStream};
use quote::{quote, quote_spanned};
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Field, ItemStruct, Token,
};

pub(crate) fn expand(item: ItemStruct) -> syn::Result<TokenStream> {
    let from_ref_impls = item
        .fields
        .iter()
        .enumerate()
        .map(|(idx, field)| expand_field(&item.ident, idx, field))
        .map(|result| match result {
            Ok(tokens) => tokens,
            Err(err) => err.into_compile_error(),
        });

    Ok(quote! {
        #(#from_ref_impls)*
    })
}

fn expand_field(state: &Ident, idx: usize, field: &Field) -> syn::Result<TokenStream> {
    let FieldAttrs { skip } = parse_attrs::<FieldAttrs>("from_ref", &field.attrs)?;

    if skip.is_some() {
        return Ok(quote! {});
    }

    let field_ty = &field.ty;
    let span = field.ty.span();

    let body = if let Some(field_ident) = &field.ident {
        quote_spanned! {span=> state.#field_ident.clone() }
    } else {
        let idx = syn::Index {
            index: idx as _,
            span: field.span(),
        };
        quote_spanned! {span=> state.#idx.clone() }
    };

    Ok(quote_spanned! {span=>
        impl ::axum::extract::FromRef<#state> for #field_ty {
            fn from_ref(state: &#state) -> Self {
                #body
            }
        }
    })
}

pub(crate) mod kw {
    syn::custom_keyword!(skip);
}

#[derive(Default)]
pub(super) struct FieldAttrs {
    pub(super) skip: Option<kw::skip>,
}

impl Parse for FieldAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut skip = None;

        while !input.is_empty() {
            let lh = input.lookahead1();
            if lh.peek(kw::skip) {
                skip = Some(input.parse()?);
            } else {
                return Err(lh.error());
            }

            let _ = input.parse::<Token![,]>();
        }

        Ok(Self { skip })
    }
}

impl Combine for FieldAttrs {
    fn combine(mut self, other: Self) -> syn::Result<Self> {
        let Self { skip } = other;
        if let Some(kw) = skip {
            if self.skip.is_some() {
                let msg = "`skip` specified more than once";
                return Err(syn::Error::new_spanned(kw, msg));
            }
            self.skip = Some(kw);
        }
        Ok(self)
    }
}
