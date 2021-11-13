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
/// # Examples
///
/// Function is not async:
///
/// ```rust,compile_fail
/// #[axum_debug::debug_handler]
/// fn handler() -> &'static str {
///     "Hello, world"
/// }
/// ```
///
/// ```text
/// error: handlers must be async functions
///   --> main.rs:xx:1
///    |
/// xx | fn handler() -> &'static str {
///    | ^^
/// ```
///
/// Wrong return type:
///
/// ```rust,compile_fail
/// #[axum_debug::debug_handler]
/// async fn handler() -> bool {
///     false
/// }
/// ```
///
/// ```text
/// error[E0277]: the trait bound `bool: IntoResponse` is not satisfied
///   --> main.rs:xx:23
///    |
/// xx | async fn handler() -> bool {
///    |                       ^^^^
///    |                       |
///    |                       the trait `IntoResponse` is not implemented for `bool`
/// ```
///
/// Wrong extractor:
///
/// ```rust,compile_fail
/// #[axum_debug::debug_handler]
/// async fn handler(a: bool) -> String {
///     format!("Can I extract a bool? {}", a)
/// }
/// ```
///
/// ```text
/// error[E0277]: the trait bound `bool: FromRequest` is not satisfied
///   --> main.rs:xx:21
///    |
/// xx | async fn handler(a: bool) -> String {
///    |                     ^^^^
///    |                     |
///    |                     the trait `FromRequest` is not implemented for `bool`
/// ```
///
/// Too many extractors:
///
/// ```rust,compile_fail
/// #[axum_debug::debug_handler]
/// async fn handler(
///     a: String,
///     b: String,
///     c: String,
///     d: String,
///     e: String,
///     f: String,
///     g: String,
///     h: String,
///     i: String,
///     j: String,
///     k: String,
///     l: String,
///     m: String,
///     n: String,
///     o: String,
///     p: String,
///     q: String,
/// ) {}
/// ```
///
/// ```text
/// error: too many extractors. 16 extractors are allowed
/// note: you can nest extractors like "a: (Extractor, Extractor), b: (Extractor, Extractor)"
///   --> main.rs:xx:5
///    |
/// xx | /     a: String,
/// xx | |     b: String,
/// xx | |     c: String,
/// xx | |     d: String,
/// ...  |
/// xx | |     p: String,
/// xx | |     q: String,
///    | |______________^
/// ```
///
/// Future is not [`Send`]:
///
/// ```rust,compile_fail
/// #[axum_debug::debug_handler]
/// async fn handler() {
///     let not_send = std::rc::Rc::new(());
///
///     async {}.await;
/// }
/// ```
///
/// ```text
/// error: future cannot be sent between threads safely
///   --> main.rs:xx:10
///    |
/// xx | async fn handler() {
///    |          ^^^^^^^
///    |          |
///    |          future returned by `handler` is not `Send`
/// ```
#[proc_macro_attribute]
pub fn debug_handler(_attr: TokenStream, input: TokenStream) -> TokenStream {
    #[cfg(not(debug_assertions))]
    return input;

    #[cfg(debug_assertions)]
    return debug::apply_debug_handler(input);
}

#[cfg(debug_assertions)]
mod debug {
    use proc_macro2::TokenStream;
    use quote::{format_ident, quote_spanned};
    use syn::{parse_macro_input, spanned::Spanned, FnArg, Ident, ItemFn, ReturnType, Signature};

    pub(crate) fn apply_debug_handler(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
        let function = parse_macro_input!(input as ItemFn);

        let vis = &function.vis;
        let sig = &function.sig;
        let ident = &sig.ident;
        let span = ident.span();
        let len = sig.inputs.len();
        let generics = create_generics(len);
        let params = sig.inputs.iter().map(|fn_arg| {
            if let FnArg::Typed(pat_type) = fn_arg {
                &pat_type.pat
            } else {
                panic!("not a handler function");
            }
        });
        let block = &function.block;

        if let Err(err) = async_check(sig) {
            return err.into_compile_error().into();
        }

        if let Err(err) = param_limit_check(sig) {
            return err.into_compile_error().into();
        }

        let check_trait = check_trait_code(sig, &generics);
        let check_return = check_return_code(sig, &generics);
        let check_params = match check_params_code(sig, &generics) {
            Ok(tokens) => tokens,
            Err(err) => return err.into_compile_error().into(),
        };

        let expanded = quote_spanned! {span=>
            #vis #sig {
                #check_trait
                #check_return
                #(#check_params)*

                #sig #block

                #ident(#(#params),*).await
            }
        };

        expanded.into()
    }

    fn create_generics(len: usize) -> Vec<Ident> {
        (1..=len).map(|i| format_ident!("T{}", i)).collect()
    }

    fn async_check(sig: &Signature) -> Result<(), syn::Error> {
        if sig.asyncness.is_none() {
            Err(syn::Error::new_spanned(
                sig.fn_token,
                "handlers must be async functions",
            ))
        } else {
            Ok(())
        }
    }

    fn param_limit_check(sig: &Signature) -> Result<(), syn::Error> {
        let max_extractors = 16;

        if sig.inputs.len() > max_extractors {
            let msg = format!(
                "too many extractors. {} extractors are allowed\n\
                note: you can nest extractors like \"a: (Extractor, Extractor), b: (Extractor, Extractor)\"",
                max_extractors
            );

            Err(syn::Error::new_spanned(&sig.inputs, msg))
        } else {
            Ok(())
        }
    }

    fn check_trait_code(sig: &Signature, generics: &[Ident]) -> proc_macro2::TokenStream {
        let ident = &sig.ident;
        let span = ident.span();

        quote_spanned! {span=>
            {
                debug_handler(#ident);

                fn debug_handler<F, Fut, #(#generics),*>(_f: F)
                where
                    F: ::std::ops::FnOnce(#(#generics),*) -> Fut + Clone + Send + Sync + 'static,
                    Fut: ::std::future::Future + Send,
                {}
            }
        }
    }

    fn check_return_code(sig: &Signature, generics: &[Ident]) -> proc_macro2::TokenStream {
        let span = match &sig.output {
            ReturnType::Default => sig.output.span(),
            ReturnType::Type(_, ty) => ty.span(),
        };
        let ident = &sig.ident;

        quote_spanned! {span=>
            {
                debug_handler(#ident);

                fn debug_handler<F, Fut, Res, #(#generics),*>(_f: F)
                where
                    F: ::std::ops::FnOnce(#(#generics),*) -> Fut,
                    Fut: ::std::future::Future<Output = Res>,
                    Res: ::axum::response::IntoResponse,
                {}
            }
        }
    }

    fn check_params_code(
        sig: &Signature,
        generics: &[Ident],
    ) -> Result<Vec<TokenStream>, syn::Error> {
        let ident = &sig.ident;
        generics
            .iter()
            .enumerate()
            .map(|(i, generic)| {
                let span = match &sig.inputs[i] {
                    FnArg::Typed(pat_type) => pat_type.ty.span(),
                    FnArg::Receiver(receiver) => {
                        // TODO: look into whether its possible to make this work
                        return Err(syn::Error::new_spanned(
                            receiver,
                            "`#[debug_handler]` is not supported on methods",
                        ));
                    }
                };

                let tokens = quote_spanned! {span=>
                    {
                        debug_handler(#ident);

                        fn debug_handler<F, Fut, #(#generics),*>(_f: F)
                        where
                            F: ::std::ops::FnOnce(#(#generics),*) -> Fut,
                            Fut: ::std::future::Future,
                            #generic: ::axum::extract::FromRequest + Send,
                        {}
                    }
                };

                Ok(tokens)
            })
            .collect()
    }
}

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/pass/*.rs");
    t.compile_fail("tests/fail/*.rs");
}
