//! Macros for [`axum`].
//!
//! [`axum`]: https://crates.io/crates/axum

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

use quote::{quote, ToTokens};
use syn::parse::Parse;

mod from_request;

/// Derive an implementation of [`FromRequest`].
///
/// Supports generating two kinds of implementations:
/// 1. One that extracts each field individually.
/// 2. Another that extracts the whole type at once via another extractor.
///
/// # Each field individually
///
/// By default `#[derive(FromRequest)]` will call `FromRequest::from_request` for each field:
///
/// ```
/// use axum_macros::FromRequest;
/// use axum::{
///     extract::{Extension, TypedHeader},
///     headers::ContentType,
///     body::Bytes,
/// };
///
/// #[derive(FromRequest)]
/// struct MyExtractor {
///     state: Extension<State>,
///     content_type: TypedHeader<ContentType>,
///     request_body: Bytes,
/// }
///
/// #[derive(Clone)]
/// struct State {
///     // ...
/// }
///
/// async fn handler(extractor: MyExtractor) {}
/// ```
///
/// This requires that each field is an extractor (ie implements [`FromRequest`]).
///
/// ## Extracting via another extractor
///
/// You can use `#[from_request(via(...))]` to extract a field via another extractor, meaning the
/// field itself doesn't need to implement `FromRequest`:
///
/// ```
/// use axum_macros::FromRequest;
/// use axum::{
///     extract::{Extension, TypedHeader},
///     headers::ContentType,
///     body::Bytes,
/// };
///
/// #[derive(FromRequest)]
/// struct MyExtractor {
///     // This will extracted via `Extension::<State>::from_request`
///     #[from_request(via(Extension))]
///     state: State,
///     // and this via `TypedHeader::<ContentType>::from_request`
///     #[from_request(via(TypedHeader))]
///     content_type: ContentType,
///     // Can still be combined with other extractors
///     request_body: Bytes,
/// }
///
/// #[derive(Clone)]
/// struct State {
///     // ...
/// }
///
/// async fn handler(extractor: MyExtractor) {}
/// ```
///
/// Note this requires the via extractor to be a generic tuple struct that implements `FromRequest`
/// and has exactly one public field:
///
/// ```
/// pub struct ViaExtractor<T>(pub T);
///
/// // impl<T, B> FromRequest<B> for ViaExtractor<T> { ... }
/// ```
///
/// More complex via extractors are not supported and require writing a manual implementation.
///
/// ## Optional fields
///
/// `#[from_request(via(...))]` supports `Option<_>` and `Result<_, _>` to make fields optional:
///
/// ```
/// use axum_macros::FromRequest;
/// use axum::{
///     extract::{TypedHeader, rejection::TypedHeaderRejection},
///     headers::{ContentType, UserAgent},
/// };
///
/// #[derive(FromRequest)]
/// struct MyExtractor {
///     // This will extracted via `Option::<TypedHeader<ContentType>>::from_request`
///     #[from_request(via(TypedHeader))]
///     content_type: Option<ContentType>,
///     // This will extracted via
///     // `Result::<TypedHeader<UserAgent>, TypedHeaderRejection>::from_request`
///     #[from_request(via(TypedHeader))]
///     user_agent: Result<UserAgent, TypedHeaderRejection>,
/// }
///
/// async fn handler(extractor: MyExtractor) {}
/// ```
///
/// ## The rejection
///
/// A rejection enum is also generated. It has a variant for each field:
///
/// ```
/// use axum_macros::FromRequest;
/// use axum::{
///     extract::{Extension, TypedHeader},
///     headers::ContentType,
///     body::Bytes,
/// };
///
/// #[derive(FromRequest)]
/// struct MyExtractor {
///     #[from_request(via(Extension))]
///     state: State,
///     #[from_request(via(TypedHeader))]
///     content_type: ContentType,
///     request_body: Bytes,
/// }
///
/// // also generates
/// //
/// // #[derive(Debug)]
/// // enum MyExtractorRejection {
/// //     State(ExtensionRejection),
/// //     ContentType(TypedHeaderRejection),
/// //     Bytes(BytesRejection),
/// // }
/// //
/// // impl axum::response::IntoResponse for MyExtractor { ... }
/// //
/// // impl std::fmt::Display for MyExtractor { ... }
/// //
/// // impl std::error::Error for MyExtractor { ... }
///
/// #[derive(Clone)]
/// struct State {
///     // ...
/// }
/// ```
///
/// The rejection's `std::error::Error::source` implementation returns the inner rejection. This
/// can be used to access source errors for example to customize rejection responses. Note this
/// means the inner rejection types must themselves implement `std::error::Error`. All extractors
/// in axum does this.
///
/// # The whole type at once
///
/// By using `#[from_request(via(...))]` on the container you can extract the whole type at once,
/// instead of each field individually:
///
/// ```
/// use axum_macros::FromRequest;
/// use axum::extract::Extension;
///
/// // This will extracted via `Extension::<State>::from_request`
/// #[derive(Clone, FromRequest)]
/// #[from_request(via(Extension))]
/// struct State {
///     // ...
/// }
///
/// async fn handler(state: State) {}
/// ```
///
/// The rejection will be the "via extractors"'s rejection. For the previous example that would be
/// [`axum::extract::rejection::ExtensionRejection`].
///
/// [`FromRequest`]: https://docs.rs/axum/latest/axum/extract/trait.FromRequest.html
/// [`axum::extract::rejection::ExtensionRejection`]: https://docs.rs/axum/latest/axum/extract/rejection/enum.ExtensionRejection.html
#[proc_macro_derive(FromRequest, attributes(from_request))]
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
        Ok(tokens) => {
            let tokens = (quote! { #tokens }).into();
            if std::env::var_os("AXUM_MACROS_DEBUG").is_some() {
                eprintln!("{}", tokens);
            }
            tokens
        }
        Err(err) => err.into_compile_error().into(),
    }
}
