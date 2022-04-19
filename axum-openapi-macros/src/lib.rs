use proc_macro::TokenTree;
use std::mem;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{Attribute, DeriveInput, FnArg, Signature, Visibility};
use syn::parse::{Parse, ParseStream};

mod json_body;
mod route;

#[proc_macro_attribute]
pub fn route(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input_ = input.clone();
    let handler_fn = syn::parse_macro_input!(input_ as route::HandlerFn);

    syn_result(route::expand(handler_fn))
}

#[proc_macro_derive(JsonBody, attributes(doc, schemars, serde, validate))]
pub fn derive_json_body(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);

    syn_result(json_body::expand(input))
}

fn syn_result(res: syn::Result<TokenStream>) -> proc_macro::TokenStream {
    res.unwrap_or_else(|e| e.into_compile_error())
        .into()
}

fn collect_docs(attrs: &[Attribute]) -> syn::Result<Vec<String>> {
    attrs.iter()
        .filter(|attr| attr.path.is_ident("doc"))
        .map(|attr| {
            parse_doc_attribute(attr.tokens.clone())
        })
        .collect()
}

fn parse_doc_attribute(tokens: TokenStream) -> syn::Result<String> {
    struct DocAttribute(String);

    impl Parse for DocAttribute {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let _ = input.parse::<syn::Token![=]>()?;
            Ok(DocAttribute(input.parse::<syn::LitStr>()?.value()))
        }
    }

    Ok(syn::parse2::<DocAttribute>(tokens)?.0)
}

