use proc_macro2::{Ident, TokenStream};
use quote::{quote, quote_spanned};
use syn::{spanned::Spanned, Field, ItemStruct};

pub(crate) fn expand(item: ItemStruct) -> TokenStream {
    let from_ref_impls = item
        .fields
        .iter()
        .enumerate()
        .map(|(idx, field)| expand_field(&item.ident, idx, field));

    quote! {
        #(#from_ref_impls)*
    }
}

fn expand_field(state: &Ident, idx: usize, field: &Field) -> TokenStream {
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

    quote_spanned! {span=>
        impl ::axum::extract::FromRef<#state> for #field_ty {
            fn from_ref(state: &#state) -> Self {
                #body
            }
        }
    }
}

#[test]
fn ui() {
    crate::run_ui_tests("from_ref");
}
