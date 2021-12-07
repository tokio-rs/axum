//! This is a debugging crate that provides better error messages for [`axum`] framework.
//!
//! While using [`axum`], you can get long error messages for simple mistakes. For example:
//!
//! ```rust,compile_fail
//! use axum::{routing::get, Router};
//!
//! #[tokio::main]
//! async fn main() {
//!     let app = Router::new().route("/", get(handler));
//!
//!     axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
//!         .serve(app.into_make_service())
//!         .await
//!         .unwrap();
//! }
//!
//! fn handler() -> &'static str {
//!     "Hello, world"
//! }
//! ```
//!
//! You will get a long error message about function not implementing [`Handler`] trait. But why
//! this function does not implement it? To figure it out [`debug_handler`] macro can be used.
//!
//! ```rust,compile_fail
//! # use axum::{routing::get, Router};
//! # use axum_debug::debug_handler;
//! #
//! # #[tokio::main]
//! # async fn main() {
//! #     let app = Router::new().route("/", get(handler));
//! #
//! #     axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
//! #         .serve(app.into_make_service())
//! #         .await
//! #         .unwrap();
//! # }
//! #
//! #[debug_handler]
//! fn handler() -> &'static str {
//!     "Hello, world"
//! }
//! ```
//!
//! ```text
//! error: handlers must be async functions
//!   --> main.rs:xx:1
//!    |
//! xx | fn handler() -> &'static str {
//!    | ^^
//! ```
//!
//! As the error message says, handler function needs to be async.
//!
//! ```rust,compile_fail
//! use axum::{routing::get, Router};
//! use axum_debug::debug_handler;
//!
//! #[tokio::main]
//! async fn main() {
//!     let app = Router::new().route("/", get(handler));
//!
//!     axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
//!         .serve(app.into_make_service())
//!         .await
//!         .unwrap();
//! }
//!
//! #[debug_handler]
//! async fn handler() -> &'static str {
//!     "Hello, world"
//! }
//! ```
//!
//! # Changing request body type
//!
//! By default `#[debug_handler]` assumes your request body type is `axum::body::Body`. This will
//! work for most extractors but, for example, it wont work for `Request<axum::body::BoxBody>`,
//! which only implements `FromRequest<BoxBody>` and _not_ `FromRequest<Body>`.
//!
//! To work around that the request body type can be customized like so:
//!
//! ```rust
//! use axum::{body::BoxBody, http::Request};
//! # use axum_debug::debug_handler;
//!
//! #[debug_handler(body = BoxBody)]
//! async fn handler(request: Request<BoxBody>) {}
//! ```
//!
//! # Performance
//!
//! Macros in this crate have no effect when using release profile. (eg. `cargo build --release`)
//!
//! [`axum`]: https://docs.rs/axum/0.3
//! [`Handler`]: https://docs.rs/axum/0.3/axum/handler/trait.Handler.html
//! [`debug_handler`]: macro@debug_handler

#![warn(
    clippy::all,
    clippy::dbg_macro,
    clippy::todo,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::mem_forget,
    clippy::unused_self,
    clippy::filter_map_next,
    clippy::needless_continue,
    clippy::needless_borrow,
    clippy::match_wildcard_for_single_variants,
    clippy::if_let_mutex,
    clippy::mismatched_target_os,
    clippy::await_holding_lock,
    clippy::match_on_vec_items,
    clippy::imprecise_flops,
    clippy::suboptimal_flops,
    clippy::lossy_float_literal,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::fn_params_excessive_bools,
    clippy::exit,
    clippy::inefficient_to_string,
    clippy::linkedlist,
    clippy::macro_use_imports,
    clippy::option_option,
    clippy::verbose_file_reads,
    clippy::unnested_or_patterns,
    clippy::str_to_string,
    rust_2018_idioms,
    future_incompatible,
    nonstandard_style,
    missing_debug_implementations,
    missing_docs
)]
#![deny(unreachable_pub, private_in_public)]
#![allow(elided_lifetimes_in_paths, clippy::type_complexity)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]

use proc_macro::TokenStream;

/// Generates better error messages when applied to a handler function.
///
/// See the [module docs](self) for more details.
#[proc_macro_attribute]
pub fn debug_handler(_attr: TokenStream, input: TokenStream) -> TokenStream {
    #[cfg(not(debug_assertions))]
    return input;

    #[cfg(debug_assertions)]
    return debug_handler::expand(_attr, input);
}

#[cfg(debug_assertions)]
mod debug_handler {
    use proc_macro2::TokenStream;
    use quote::{format_ident, quote, quote_spanned};
    use syn::{parse::Parse, spanned::Spanned, FnArg, ItemFn, Token, Type};

    pub(crate) fn expand(
        attr: proc_macro::TokenStream,
        input: proc_macro::TokenStream,
    ) -> proc_macro::TokenStream {
        match try_expand(attr.into(), input.into()) {
            Ok(tokens) => tokens.into(),
            Err(err) => err.into_compile_error().into(),
        }
    }

    pub(crate) fn try_expand(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
        let attr = syn::parse2::<Attrs>(attr)?;
        let item_fn = syn::parse2::<ItemFn>(input.clone())?;

        check_extractor_count(&item_fn)?;

        let check_inputs_impls_from_request =
            check_inputs_impls_from_request(&item_fn, &attr.body_ty);
        let check_output_impls_into_response = check_output_impls_into_response(&item_fn);
        let check_future_send = check_future_send(&item_fn);

        let tokens = quote! {
            #input
            #check_inputs_impls_from_request
            #check_output_impls_into_response
            #check_future_send
        };

        Ok(tokens)
    }

    struct Attrs {
        body_ty: Type,
    }

    impl Parse for Attrs {
        fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
            let mut body_ty = None;

            while !input.is_empty() {
                let ident = input.parse::<syn::Ident>()?;
                if ident == "body" {
                    input.parse::<Token![=]>()?;
                    body_ty = Some(input.parse()?);
                } else {
                    return Err(syn::Error::new_spanned(ident, "unknown argument"));
                }

                let _ = input.parse::<Token![,]>();
            }

            let body_ty = body_ty.unwrap_or_else(|| syn::parse_quote!(axum::body::Body));

            Ok(Self { body_ty })
        }
    }

    fn check_extractor_count(item_fn: &ItemFn) -> syn::Result<()> {
        let max_extractors = 16;
        if item_fn.sig.inputs.len() <= max_extractors {
            Ok(())
        } else {
            Err(syn::Error::new_spanned(
                &item_fn.sig.inputs,
                format!(
                    "Handlers cannot take more than {} arguments. Use `(a, b): (ExtractorA, ExtractorA)` to further nest extractors",
                    max_extractors,
                )
            ))
        }
    }

    fn check_inputs_impls_from_request(item_fn: &ItemFn, body_ty: &Type) -> TokenStream {
        if !item_fn.sig.generics.params.is_empty() {
            return syn::Error::new_spanned(
                &item_fn.sig.generics,
                "`#[axum_debug::debug_handler]` doesn't support generic functions",
            )
            .into_compile_error();
        }

        item_fn
            .sig
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, arg)| {
                let (span, ty) = match arg {
                    FnArg::Receiver(receiver) => {
                        if receiver.reference.is_some() {
                            return syn::Error::new_spanned(
                                receiver,
                                "Handlers must only take owned values",
                            )
                            .into_compile_error();
                        }

                        let span = receiver.span();
                        (span, syn::parse_quote!(Self))
                    }
                    FnArg::Typed(typed) => {
                        let ty = &typed.ty;
                        let span = ty.span();
                        (span, ty.clone())
                    }
                };

                let name = format_ident!(
                    "__axum_debug_check_{}_{}_from_request",
                    item_fn.sig.ident,
                    idx
                );
                quote_spanned! {span=>
                    #[allow(warnings)]
                    fn #name()
                    where
                        #ty: ::axum::extract::FromRequest<#body_ty> + Send,
                    {}
                }
            })
            .collect::<TokenStream>()
    }

    fn check_output_impls_into_response(item_fn: &ItemFn) -> TokenStream {
        let ty = match &item_fn.sig.output {
            syn::ReturnType::Default => return quote! {},
            syn::ReturnType::Type(_, ty) => ty,
        };
        let span = ty.span();

        let declare_inputs = item_fn
            .sig
            .inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Receiver(_) => None,
                FnArg::Typed(pat_ty) => {
                    let pat = &pat_ty.pat;
                    let ty = &pat_ty.ty;
                    Some(quote! {
                        let #pat: #ty = panic!();
                    })
                }
            })
            .collect::<TokenStream>();

        let block = &item_fn.block;

        let make_value_name = format_ident!(
            "__axum_debug_check_{}_into_response_make_value",
            item_fn.sig.ident
        );

        let make = if item_fn.sig.asyncness.is_some() {
            quote_spanned! {span=>
                #[allow(warnings)]
                async fn #make_value_name() -> #ty {
                    #declare_inputs
                    #block
                }
            }
        } else {
            quote_spanned! {span=>
                #[allow(warnings)]
                fn #make_value_name() -> #ty {
                    #declare_inputs
                    #block
                }
            }
        };

        let name = format_ident!("__axum_debug_check_{}_into_response", item_fn.sig.ident);

        if let Some(receiver) = self_receiver(item_fn) {
            quote_spanned! {span=>
                #make

                #[allow(warnings)]
                async fn #name() {
                    let value = #receiver #make_value_name().await;
                    fn check<T>(_: T)
                        where T: ::axum::response::IntoResponse
                    {}
                    check(value);
                }
            }
        } else {
            quote_spanned! {span=>
                #[allow(warnings)]
                async fn #name() {
                    #make

                    let value = #make_value_name().await;

                    fn check<T>(_: T)
                    where T: ::axum::response::IntoResponse
                    {}

                    check(value);
                }
            }
        }
    }

    fn check_future_send(item_fn: &ItemFn) -> TokenStream {
        if item_fn.sig.asyncness.is_none() {
            match &item_fn.sig.output {
                syn::ReturnType::Default => {
                    return syn::Error::new_spanned(
                        &item_fn.sig.fn_token,
                        "Handlers must be `async fn`s",
                    )
                    .into_compile_error();
                }
                syn::ReturnType::Type(_, ty) => ty,
            };
        }

        let span = item_fn.span();

        let handler_name = &item_fn.sig.ident;

        let args = item_fn.sig.inputs.iter().map(|_| {
            quote_spanned! {span=> panic!() }
        });

        let name = format_ident!("__axum_debug_check_{}_future", item_fn.sig.ident);

        if let Some(receiver) = self_receiver(item_fn) {
            quote_spanned! {span=>
                #[allow(warnings)]
                fn #name() {
                    let future = #receiver #handler_name(#(#args),*);
                    fn check<T>(_: T)
                        where T: ::std::future::Future + Send
                    {}
                    check(future);
                }
            }
        } else {
            quote_spanned! {span=>
                #[allow(warnings)]
                fn #name() {
                    #item_fn

                    let future = #handler_name(#(#args),*);
                    fn check<T>(_: T)
                        where T: ::std::future::Future + Send
                    {}
                    check(future);
                }
            }
        }
    }

    fn self_receiver(item_fn: &ItemFn) -> Option<TokenStream> {
        let takes_self = item_fn
            .sig
            .inputs
            .iter()
            .any(|arg| matches!(arg, syn::FnArg::Receiver(_)));
        if takes_self {
            return Some(quote! { Self:: });
        }

        if let syn::ReturnType::Type(_, ty) = &item_fn.sig.output {
            if let syn::Type::Path(path) = &**ty {
                let segments = &path.path.segments;
                if segments.len() == 1 {
                    if let Some(last) = segments.last() {
                        match &last.arguments {
                            syn::PathArguments::None if last.ident == "Self" => {
                                return Some(quote! { Self:: });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        None
    }
}

#[test]
fn ui() {
    #[rustversion::stable]
    fn go() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/fail/*.rs");
        t.pass("tests/pass/*.rs");
    }

    #[rustversion::not(stable)]
    fn go() {}

    go();
}
