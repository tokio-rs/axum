//! Procedural macros for [`axum-debug`] crate.
//!
//! [`axum-debug`]: https://crates.io/crates/axum-debug

#![warn(
    clippy::all,
    clippy::dbg_macro,
    clippy::todo,
    clippy::mem_forget,
    rust_2018_idioms,
    future_incompatible,
    nonstandard_style,
    missing_debug_implementations,
    missing_docs
)]
#![deny(unreachable_pub, private_in_public)]
#![forbid(unsafe_code)]

use proc_macro::TokenStream;

/// Generates better error messages when applied to a handler function.
///
/// # Examples
///
/// Function is not async:
///
/// ```rust,ignore
/// #[debug_handler]
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
/// ```rust,ignore
/// #[debug_handler]
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
/// ```rust,ignore
/// #[debug_handler]
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
/// ```rust,ignore
/// #[debug_handler]
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
/// ```rust,ignore
/// #[debug_handler]
/// async fn handler() {
///     let not_send = std::rc::Rc::new(());
///
///     async{}.await;
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
///
/// [`Send`]: Send
#[proc_macro_attribute]
pub fn debug_handler(_attr: TokenStream, input: TokenStream) -> TokenStream {
    #[cfg(not(debug_assertions))]
    return input;

    #[cfg(debug_assertions)]
    return debug::apply_debug_handler(input);
}

#[cfg(debug_assertions)]
mod debug {
    use proc_macro::TokenStream;
    use proc_macro2::Span;
    use quote::quote_spanned;
    use syn::{parse_macro_input, FnArg, Ident, ItemFn, ReturnType, Signature};

    pub(crate) fn apply_debug_handler(input: TokenStream) -> TokenStream {
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

        if let Err(error) = async_check(sig) {
            return error;
        }

        if let Err(error) = param_limit_check(sig) {
            return error;
        }

        let check_trait = check_trait_code(sig, &generics);
        let check_return = check_return_code(sig, &generics);
        let check_params = check_params_code(sig, &generics);

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
        let mut vec = Vec::new();
        for i in 1..=len {
            vec.push(Ident::new(&format!("T{}", i), Span::call_site()));
        }
        vec
    }

    fn async_check(sig: &Signature) -> Result<(), TokenStream> {
        if sig.asyncness.is_none() {
            let error = syn::Error::new_spanned(sig.fn_token, "handlers must be async functions")
                .to_compile_error()
                .into();

            return Err(error);
        }

        Ok(())
    }

    fn param_limit_check(sig: &Signature) -> Result<(), TokenStream> {
        if sig.inputs.len() > 16 {
            let msg = "too many extractors. 16 extractors are allowed\n\
                       note: you can nest extractors like \"a: (Extractor, Extractor), b: (Extractor, Extractor)\"";

            let error = syn::Error::new_spanned(&sig.inputs, msg)
                .to_compile_error()
                .into();

            return Err(error);
        }

        Ok(())
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
            ReturnType::Default => syn::Error::new_spanned(&sig.output, "").span(),
            ReturnType::Type(_, t) => syn::Error::new_spanned(t, "").span(),
        };
        let ident = &sig.ident;

        quote_spanned! {span=>
            {
                debug_handler(#ident);

                fn debug_handler<F, Fut, Res, #(#generics),*>(_f: F)
                where
                    F: ::std::ops::FnOnce(#(#generics),*) -> Fut,
                    Fut: ::std::future::Future<Output = Res>,
                    Res: ::axum_debug::axum::response::IntoResponse,
                {}
            }
        }
    }

    fn check_params_code(sig: &Signature, generics: &[Ident]) -> Vec<proc_macro2::TokenStream> {
        let mut vec = Vec::new();

        let ident = &sig.ident;

        for (i, generic) in generics.iter().enumerate() {
            let span = match &sig.inputs[i] {
                FnArg::Typed(pat_type) => syn::Error::new_spanned(&pat_type.ty, "").span(),
                _ => panic!("not a handler"),
            };

            let token_stream = quote_spanned! {span=>
                {
                    debug_handler(#ident);

                    fn debug_handler<F, Fut, #(#generics),*>(_f: F)
                    where
                        F: ::std::ops::FnOnce(#(#generics),*) -> Fut,
                        Fut: ::std::future::Future,
                        #generic: ::axum_debug::axum::extract::FromRequest + Send,
                    {}
                }
            };

            vec.push(token_stream);
        }

        vec
    }
}
