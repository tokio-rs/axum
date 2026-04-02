use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{parse::Parse, spanned::Spanned, ItemStruct};

use super::attr_parsing::Combine;

pub(crate) fn expand(item_struct: &ItemStruct) -> syn::Result<TokenStream> {
    let ItemStruct {
        attrs,
        ident,
        generics,
        ..
    } = &item_struct;

    if !generics.params.is_empty() || generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            generics,
            "`#[derive(TypedMethod)]` doesn't support generics",
        ));
    }

    let Attrs { method_filter } = super::attr_parsing::parse_attrs("typed_method", attrs)?;

    let method_filter = method_filter.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "Missing method filter: `#[typed_method(\"GET\")]`",
        )
    })?;

    let typed_path_impl = quote_spanned! {method_filter.span()=>
        #[automatically_derived]
        impl ::axum_typed_method::TypedMethod for #ident {
            const METHOD: ::axum::routing::MethodFilter = #method_filter;
        }
    };

    Ok(quote! (#typed_path_impl))
}

#[derive(Default)]
struct Attrs {
    method_filter: Option<syn::Path>,
}

impl Parse for Attrs {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            method_filter: Some(input.parse()?),
        })
    }
}

impl Combine for Attrs {
    fn combine(mut self, other: Self) -> syn::Result<Self> {
        let Self { method_filter } = other;
        if let Some(method_filter) = method_filter {
            if self.method_filter.is_some() {
                return Err(syn::Error::new_spanned(
                    method_filter,
                    "method filter specified more than once",
                ));
            }
            self.method_filter = Some(method_filter);
        }

        Ok(self)
    }
}

#[test]
fn ui() {
    crate::run_ui_tests("typed_method");
}
