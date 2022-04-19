use proc_macro2::TokenStream;

use std::mem;

pub fn expand(mut input: syn::DeriveInput) -> syn::Result<TokenStream> {
    let docs = dbg!(super::collect_docs(&input.attrs))?;

    let shadowed_ident = quote::format_ident!("__{}", input.ident);

    let name = mem::replace(&mut input.ident, shadowed_ident);
    let shadowed_ident = &input.ident;
    let description = docs.iter().map(|s| s.trim());

    Ok(quote::quote! {
        impl axum_openapi::JsonBody for #name {
            fn description() -> &'static str {
                concat!(#(#description, "\n"),*).trim()
            }

            fn json_schema() -> axum_openapi::__macro_reexport::schemars::schema::RootSchema {
                #[derive(axum_openapi::__macro_reexport::schemars::JsonSchema)]
                #[schemars(crate = "axum_openapi::__macro_reexport::schemars")]
                #input

                axum_openapi::__macro_reexport::schemars::schema_for!(#shadowed_ident)
            }
        }
    })
}