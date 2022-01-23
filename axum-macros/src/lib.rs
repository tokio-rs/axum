use quote::{quote, ToTokens};
use syn::parse::Parse;

mod from_request;

#[proc_macro_derive(FromRequest)]
pub fn derive_from_request(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand_with(item, from_request::expand)
}

fn expand_with<F, T, K>(input: proc_macro::TokenStream, f: F) -> proc_macro::TokenStream
where
    F: FnOnce(T) -> syn::Result<K>,
    T: Parse,
    K: ToTokens,
{
    match syn::parse(input).and_then(f) {
        Ok(tokens) => (quote! { #tokens }).into(),
        Err(err) => err.into_compile_error().into(),
    }
}
