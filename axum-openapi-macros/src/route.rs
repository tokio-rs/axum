use std::mem;

use proc_macro2::TokenStream;
use syn::{Attribute, FnArg, Signature, Visibility};
use syn::parse::{Parse, ParseStream};

// We only care about a function's attributes and signature, the rest we pass on verbatim.
#[derive(Debug)]
pub struct HandlerFn {
    attrs: Vec<Attribute>,
    vis: Visibility,
    sig: Signature,
    body: TokenStream
}

pub fn expand(handler_fn: HandlerFn) -> syn::Result<TokenStream> {
    let docs = super::collect_docs(&handler_fn.attrs)?;

    let HandlerFn { attrs, vis, mut sig, body } = handler_fn;

    let name = mem::replace(&mut sig.ident, syn::parse_quote! { inner });

    let arg_types = sig.inputs.iter()
        .map(|arg| match arg {
            FnArg::Receiver(recv) => Err(syn::Error::new_spanned(recv, "handler function cannot have a receiver")),
            FnArg::Typed(pat_ty) => Ok(&*pat_ty.ty)
        })
        .collect::<syn::Result<Vec<&syn::Type>>>()?;

    let handler_type = quote::quote! {
        impl axum_openapi::__macro_reexport::Handler<
            (#(#arg_types),*,),
            Future = axum_openapi::__macro_reexport::BoxFuture<'static, axum_openapi::__macro_reexport::Response>
        >
    };

    let summary = docs.get(0).map_or("", |s| s.trim());
    let description = docs.iter().map(|s| s.trim());

    let operation = quote::quote! {
        let mut operation = axum_openapi::openapi::Operation {
            // TODO: pass tags in via `args`?
            tags: &[],
            summary: #summary,
            description: concat!(#(#description, "\n"),*).trim(),
            operation_id: concat!(module_path!(), "::", stringify!(#name)),
            parameters: vec![],
            request_body: None,
        };

        #(operation.__push_handler_arg::<#arg_types>();)*

        operation
    };

    let output = quote::quote! {
        #(#attrs)*
        #vis fn #name() -> axum_openapi::RouteHandler<#handler_type> {
            #sig #body

            axum_openapi::RouteHandler {
                handler: inner,
                describe: || {
                    #operation
                },
            }
        }
    };

    Ok(output)
}


impl Parse for HandlerFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Same as `ItemFn::parse()` except we don't parse the body
        let outer_attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        let sig: Signature = input.parse()?;
        // Note: this produces an error because it doesn't fully consume the input.
        // let body = input.cursor().token_stream();
        let body: TokenStream = input.parse()?;

        Ok(HandlerFn {
            attrs: outer_attrs,
            vis,
            sig,
            body
        })
    }
}

