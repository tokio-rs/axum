//! Macros for [`axum`].
//!
//! [`axum`]: https://crates.io/crates/axum

#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(not(test), warn(clippy::print_stdout, clippy::dbg_macro))]

use debug_handler::FunctionKind;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse::Parse, Type};

mod attr_parsing;
#[cfg(feature = "__private")]
mod axum_test;
mod debug_handler;
mod from_ref;
mod from_request;
mod typed_path;
mod with_position;

use from_request::Trait::{FromRequest, FromRequestParts};

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
///     extract::Extension,
///     body::Bytes,
/// };
/// use axum_extra::{
///     TypedHeader,
///     headers::ContentType,
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
/// This requires that each field is an extractor (i.e. implements [`FromRequest`]).
///
/// Note that only the last field can consume the request body. Therefore this doesn't compile:
///
/// ```compile_fail
/// use axum_macros::FromRequest;
/// use axum::body::Bytes;
///
/// #[derive(FromRequest)]
/// struct MyExtractor {
///     // only the last field can implement `FromRequest`
///     // other fields must only implement `FromRequestParts`
///     bytes: Bytes,
///     string: String,
/// }
/// ```
///
/// ## Extracting via another extractor
///
/// You can use `#[from_request(via(...))]` to extract a field via another extractor, meaning the
/// field itself doesn't need to implement `FromRequest`:
///
/// ```
/// use axum_macros::FromRequest;
/// use axum::{
///     extract::Extension,
///     body::Bytes,
/// };
/// use axum_extra::{
///     TypedHeader,
///     headers::ContentType,
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
/// Note this requires the via extractor to be a generic newtype struct (a tuple struct with
/// exactly one public field) that implements `FromRequest`:
///
/// ```
/// pub struct ViaExtractor<T>(pub T);
///
/// // impl<T, S> FromRequest<S> for ViaExtractor<T> { ... }
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
/// use axum_extra::{
///     TypedHeader,
///     headers::{ContentType, UserAgent},
///     typed_header::TypedHeaderRejection,
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
/// By default [`axum::response::Response`] will be used as the rejection. You can also use your own
/// rejection type with `#[from_request(rejection(YourType))]`:
///
/// ```
/// use axum::{
///     extract::{
///         rejection::{ExtensionRejection, StringRejection},
///         FromRequest,
///     },
///     Extension,
///     response::{Response, IntoResponse},
/// };
///
/// #[derive(FromRequest)]
/// #[from_request(rejection(MyRejection))]
/// struct MyExtractor {
///     state: Extension<String>,
///     body: String,
/// }
///
/// struct MyRejection(Response);
///
/// // This tells axum how to convert `Extension`'s rejections into `MyRejection`
/// impl From<ExtensionRejection> for MyRejection {
///     fn from(rejection: ExtensionRejection) -> Self {
///         // ...
///         # todo!()
///     }
/// }
///
/// // This tells axum how to convert `String`'s rejections into `MyRejection`
/// impl From<StringRejection> for MyRejection {
///     fn from(rejection: StringRejection) -> Self {
///         // ...
///         # todo!()
///     }
/// }
///
/// // All rejections must implement `IntoResponse`
/// impl IntoResponse for MyRejection {
///     fn into_response(self) -> Response {
///         self.0
///     }
/// }
/// ```
///
/// ## Concrete state
///
/// If the extraction can be done only for a concrete state, that type can be specified with
/// `#[from_request(state(YourState))]`:
///
/// ```
/// use axum::extract::{FromRequest, FromRequestParts};
///
/// #[derive(Clone)]
/// struct CustomState;
///
/// struct MyInnerType;
///
/// impl FromRequestParts<CustomState> for MyInnerType {
///     // ...
///     # type Rejection = ();
///
///     # async fn from_request_parts(
///         # _parts: &mut axum::http::request::Parts,
///         # _state: &CustomState
///     # ) -> Result<Self, Self::Rejection> {
///     #    todo!()
///     # }
/// }
///
/// #[derive(FromRequest)]
/// #[from_request(state(CustomState))]
/// struct MyExtractor {
///     custom: MyInnerType,
///     body: String,
/// }
/// ```
///
/// This is not needed for a `State<T>` as the type is inferred in that case.
///
/// ```
/// use axum::extract::{FromRequest, FromRequestParts, State};
///
/// #[derive(Clone)]
/// struct CustomState;
///
/// #[derive(FromRequest)]
/// struct MyExtractor {
///     custom: State<CustomState>,
///     body: String,
/// }
/// ```
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
/// You can use a different rejection type with `#[from_request(rejection(YourType))]`:
///
/// ```
/// use axum_macros::FromRequest;
/// use axum::{
///     extract::{Extension, rejection::ExtensionRejection},
///     response::{IntoResponse, Response},
///     Json,
///     http::StatusCode,
/// };
/// use serde_json::json;
///
/// // This will extracted via `Extension::<State>::from_request`
/// #[derive(Clone, FromRequest)]
/// #[from_request(
///     via(Extension),
///     // Use your own rejection type
///     rejection(MyRejection),
/// )]
/// struct State {
///     // ...
/// }
///
/// struct MyRejection(Response);
///
/// // This tells axum how to convert `Extension`'s rejections into `MyRejection`
/// impl From<ExtensionRejection> for MyRejection {
///     fn from(rejection: ExtensionRejection) -> Self {
///         let response = (
///             StatusCode::INTERNAL_SERVER_ERROR,
///             Json(json!({ "error": "Something went wrong..." })),
///         ).into_response();
///
///         MyRejection(response)
///     }
/// }
///
/// // All rejections must implement `IntoResponse`
/// impl IntoResponse for MyRejection {
///     fn into_response(self) -> Response {
///         self.0
///     }
/// }
///
/// async fn handler(state: State) {}
/// ```
///
/// This allows you to wrap other extractors and easily customize the rejection:
///
/// ```
/// use axum_macros::FromRequest;
/// use axum::{
///     extract::{Extension, rejection::JsonRejection},
///     response::{IntoResponse, Response},
///     http::StatusCode,
/// };
/// use serde_json::json;
/// use serde::Deserialize;
///
/// // create an extractor that internally uses `axum::Json` but has a custom rejection
/// #[derive(FromRequest)]
/// #[from_request(via(axum::Json), rejection(MyRejection))]
/// struct MyJson<T>(T);
///
/// struct MyRejection(Response);
///
/// impl From<JsonRejection> for MyRejection {
///     fn from(rejection: JsonRejection) -> Self {
///         let response = (
///             StatusCode::INTERNAL_SERVER_ERROR,
///             axum::Json(json!({ "error": rejection.to_string() })),
///         ).into_response();
///
///         MyRejection(response)
///     }
/// }
///
/// impl IntoResponse for MyRejection {
///     fn into_response(self) -> Response {
///         self.0
///     }
/// }
///
/// #[derive(Deserialize)]
/// struct Payload {}
///
/// async fn handler(
///     // make sure to use `MyJson` and not `axum::Json`
///     MyJson(payload): MyJson<Payload>,
/// ) {}
/// ```
///
/// # Known limitations
///
/// Generics are only supported on tuple structs with exactly one field. Thus this doesn't work
///
/// ```compile_fail
/// #[derive(axum_macros::FromRequest)]
/// struct MyExtractor<T> {
///     thing: Option<T>,
/// }
/// ```
///
/// [`FromRequest`]: https://docs.rs/axum/0.8/axum/extract/trait.FromRequest.html
/// [`axum::response::Response`]: https://docs.rs/axum/0.8/axum/response/type.Response.html
/// [`axum::extract::rejection::ExtensionRejection`]: https://docs.rs/axum/0.8/axum/extract/rejection/enum.ExtensionRejection.html
#[proc_macro_derive(FromRequest, attributes(from_request))]
pub fn derive_from_request(item: TokenStream) -> TokenStream {
    expand_with(item, |item| from_request::expand(item, FromRequest))
}

/// Derive an implementation of [`FromRequestParts`].
///
/// This works similarly to `#[derive(FromRequest)]` except it uses [`FromRequestParts`]. All the
/// same options are supported.
///
/// # Example
///
/// ```
/// use axum_macros::FromRequestParts;
/// use axum::{
///     extract::Query,
/// };
/// use axum_extra::{
///     TypedHeader,
///     headers::ContentType,
/// };
/// use std::collections::HashMap;
///
/// #[derive(FromRequestParts)]
/// struct MyExtractor {
///     #[from_request(via(Query))]
///     query_params: HashMap<String, String>,
///     content_type: TypedHeader<ContentType>,
/// }
///
/// async fn handler(extractor: MyExtractor) {}
/// ```
///
/// # Cannot extract the body
///
/// [`FromRequestParts`] cannot extract the request body:
///
/// ```compile_fail
/// use axum_macros::FromRequestParts;
///
/// #[derive(FromRequestParts)]
/// struct MyExtractor {
///     body: String,
/// }
/// ```
///
/// Use `#[derive(FromRequest)]` for that.
///
/// [`FromRequestParts`]: https://docs.rs/axum/0.8/axum/extract/trait.FromRequestParts.html
#[proc_macro_derive(FromRequestParts, attributes(from_request))]
pub fn derive_from_request_parts(item: TokenStream) -> TokenStream {
    expand_with(item, |item| from_request::expand(item, FromRequestParts))
}

/// Generates better error messages when applied to handler functions.
///
/// While using [`axum`], you can get long error messages for simple mistakes. For example:
///
/// ```compile_fail
/// use axum::{routing::get, Router};
///
/// #[tokio::main]
/// async fn main() {
///     let app = Router::new().route("/", get(handler));
///
///     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
///     axum::serve(listener, app).await.unwrap();
/// }
///
/// fn handler() -> &'static str {
///     "Hello, world"
/// }
/// ```
///
/// You will get a long error message about function not implementing [`Handler`] trait. But why
/// does this function not implement it? To figure it out, the [`debug_handler`] macro can be used.
///
/// ```compile_fail
/// # use axum::{routing::get, Router};
/// # use axum_macros::debug_handler;
/// #
/// # #[tokio::main]
/// # async fn main() {
/// #     let app = Router::new().route("/", get(handler));
/// #
/// #     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
/// #     axum::serve(listener, app).await.unwrap();
/// # }
/// #
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
/// As the error message says, handler function needs to be async.
///
/// ```no_run
/// use axum::{routing::get, Router, debug_handler};
///
/// #[tokio::main]
/// async fn main() {
///     let app = Router::new().route("/", get(handler));
///
///     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
///     axum::serve(listener, app).await.unwrap();
/// }
///
/// #[debug_handler]
/// async fn handler() -> &'static str {
///     "Hello, world"
/// }
/// ```
///
/// # Changing state type
///
/// By default `#[debug_handler]` assumes your state type is `()` unless your handler has a
/// [`axum::extract::State`] argument:
///
/// ```
/// use axum::{debug_handler, extract::State};
///
/// #[debug_handler]
/// async fn handler(
///     // this makes `#[debug_handler]` use `AppState`
///     State(state): State<AppState>,
/// ) {}
///
/// #[derive(Clone)]
/// struct AppState {}
/// ```
///
/// If your handler takes multiple [`axum::extract::State`] arguments or you need to otherwise
/// customize the state type you can set it with `#[debug_handler(state = ...)]`:
///
/// ```
/// use axum::{debug_handler, extract::{State, FromRef}};
///
/// #[debug_handler(state = AppState)]
/// async fn handler(
///     State(app_state): State<AppState>,
///     State(inner_state): State<InnerState>,
/// ) {}
///
/// #[derive(Clone)]
/// struct AppState {
///     inner: InnerState,
/// }
///
/// #[derive(Clone)]
/// struct InnerState {}
///
/// impl FromRef<AppState> for InnerState {
///     fn from_ref(state: &AppState) -> Self {
///         state.inner.clone()
///     }
/// }
/// ```
///
/// # Limitations
///
/// This macro does not work for functions in an `impl` block that don't have a `self` parameter:
///
/// ```compile_fail
/// use axum::{debug_handler, extract::Path};
///
/// struct App {}
///
/// impl App {
///     #[debug_handler]
///     async fn handler(Path(_): Path<String>) {}
/// }
/// ```
///
/// This will yield an error similar to this:
///
/// ```text
/// error[E0425]: cannot find function `__axum_macros_check_handler_0_from_request_check` in this scope
//    --> src/main.rs:xx:xx
//     |
//  xx |     pub async fn handler(Path(_): Path<String>)  {}
//     |                                   ^^^^ not found in this scope
/// ```
///
/// # Performance
///
/// This macro has no effect when compiled with the release profile. (eg. `cargo build --release`)
///
/// [`axum`]: https://docs.rs/axum/0.8
/// [`Handler`]: https://docs.rs/axum/0.8/axum/handler/trait.Handler.html
/// [`axum::extract::State`]: https://docs.rs/axum/0.8/axum/extract/struct.State.html
/// [`debug_handler`]: macro@debug_handler
#[proc_macro_attribute]
pub fn debug_handler(_attr: TokenStream, input: TokenStream) -> TokenStream {
    #[cfg(not(debug_assertions))]
    return input;

    #[cfg(debug_assertions)]
    return expand_attr_with(_attr, input, |attrs, item_fn| {
        debug_handler::expand(attrs, item_fn, FunctionKind::Handler)
    });
}

/// Generates better error messages when applied to middleware functions.
///
/// This works similarly to [`#[debug_handler]`](macro@debug_handler) except for middleware using
/// [`axum::middleware::from_fn`].
///
/// # Example
///
/// ```no_run
/// use axum::{
///     routing::get,
///     extract::Request,
///     response::Response,
///     Router,
///     middleware::{self, Next},
///     debug_middleware,
/// };
///
/// #[tokio::main]
/// async fn main() {
///     let app = Router::new()
///         .route("/", get(|| async {}))
///         .layer(middleware::from_fn(my_middleware));
///
///     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
///     axum::serve(listener, app).await.unwrap();
/// }
///
/// // if this wasn't a valid middleware function #[debug_middleware] would
/// // improve compile error
/// #[debug_middleware]
/// async fn my_middleware(
///     request: Request,
///     next: Next,
/// ) -> Response {
///     next.run(request).await
/// }
/// ```
///
/// # Performance
///
/// This macro has no effect when compiled with the release profile. (eg. `cargo build --release`)
///
/// [`axum`]: https://docs.rs/axum/latest
/// [`axum::middleware::from_fn`]: https://docs.rs/axum/0.8/axum/middleware/fn.from_fn.html
/// [`debug_middleware`]: macro@debug_middleware
#[proc_macro_attribute]
pub fn debug_middleware(_attr: TokenStream, input: TokenStream) -> TokenStream {
    #[cfg(not(debug_assertions))]
    return input;

    #[cfg(debug_assertions)]
    return expand_attr_with(_attr, input, |attrs, item_fn| {
        debug_handler::expand(attrs, item_fn, FunctionKind::Middleware)
    });
}

/// Private API: Do no use this!
///
/// Attribute macro to be placed on test functions that'll generate two functions:
///
/// 1. One identical to the function it was placed on.
/// 2. One where calls to `Router::nest` has been replaced with `Router::nest_service`
///
/// This makes it easy to that `nest` and `nest_service` behaves in the same way, without having to
/// manually write identical tests for both methods.
#[cfg(feature = "__private")]
#[proc_macro_attribute]
#[doc(hidden)]
pub fn __private_axum_test(_attr: TokenStream, input: TokenStream) -> TokenStream {
    expand_attr_with(_attr, input, axum_test::expand)
}

/// Derive an implementation of [`axum_extra::routing::TypedPath`].
///
/// See that trait for more details.
///
/// [`axum_extra::routing::TypedPath`]: https://docs.rs/axum-extra/latest/axum_extra/routing/trait.TypedPath.html
#[proc_macro_derive(TypedPath, attributes(typed_path))]
pub fn derive_typed_path(input: TokenStream) -> TokenStream {
    expand_with(input, typed_path::expand)
}

/// Derive an implementation of [`FromRef`] for each field in a struct.
///
/// # Example
///
/// ```
/// use axum::{
///     Router,
///     routing::get,
///     extract::{State, FromRef},
/// };
///
/// #
/// # type AuthToken = String;
/// # type DatabasePool = ();
/// #
/// // This will implement `FromRef` for each field in the struct.
/// #[derive(FromRef, Clone)]
/// struct AppState {
///     auth_token: AuthToken,
///     database_pool: DatabasePool,
///     // fields can also be skipped
///     #[from_ref(skip)]
///     api_token: String,
/// }
///
/// // So those types can be extracted via `State`
/// async fn handler(State(auth_token): State<AuthToken>) {}
///
/// async fn other_handler(State(database_pool): State<DatabasePool>) {}
///
/// # let auth_token = Default::default();
/// # let database_pool = Default::default();
/// let state = AppState {
///     auth_token,
///     database_pool,
///     api_token: "secret".to_owned(),
/// };
///
/// let app = Router::new()
///     .route("/", get(handler).post(other_handler))
///     .with_state(state);
/// # let _: axum::Router = app;
/// ```
///
/// [`FromRef`]: https://docs.rs/axum/0.8/axum/extract/trait.FromRef.html
#[proc_macro_derive(FromRef, attributes(from_ref))]
pub fn derive_from_ref(item: TokenStream) -> TokenStream {
    expand_with(item, from_ref::expand)
}

fn expand_with<F, I, K>(input: TokenStream, f: F) -> TokenStream
where
    F: FnOnce(I) -> syn::Result<K>,
    I: Parse,
    K: ToTokens,
{
    expand(syn::parse(input).and_then(f))
}

fn expand_attr_with<F, A, I, K>(attr: TokenStream, input: TokenStream, f: F) -> TokenStream
where
    F: FnOnce(A, I) -> K,
    A: Parse,
    I: Parse,
    K: ToTokens,
{
    let expand_result = (|| {
        let attr = syn::parse(attr)?;
        let input = syn::parse(input)?;
        Ok(f(attr, input))
    })();
    expand(expand_result)
}

fn expand<T>(result: syn::Result<T>) -> TokenStream
where
    T: ToTokens,
{
    match result {
        Ok(tokens) => {
            let tokens = (quote! { #tokens }).into();
            if std::env::var_os("AXUM_MACROS_DEBUG").is_some() {
                eprintln!("{tokens}");
            }
            tokens
        }
        Err(err) => err.into_compile_error().into(),
    }
}

fn infer_state_types<'a, I>(types: I) -> impl Iterator<Item = Type> + 'a
where
    I: Iterator<Item = &'a Type> + 'a,
{
    types
        .filter_map(|ty| {
            if let Type::Path(path) = ty {
                Some(&path.path)
            } else {
                None
            }
        })
        .filter_map(|path| {
            if let Some(last_segment) = path.segments.last() {
                if last_segment.ident != "State" {
                    return None;
                }

                match &last_segment.arguments {
                    syn::PathArguments::AngleBracketed(args) if args.args.len() == 1 => {
                        Some(args.args.first().unwrap())
                    }
                    _ => None,
                }
            } else {
                None
            }
        })
        .filter_map(|generic_arg| {
            if let syn::GenericArgument::Type(ty) = generic_arg {
                Some(ty)
            } else {
                None
            }
        })
        .cloned()
}

#[cfg(test)]
fn run_ui_tests(directory: &str) {
    #[rustversion::nightly]
    fn go(directory: &str) {
        let t = trybuild::TestCases::new();

        if let Ok(mut path) = std::env::var("AXUM_TEST_ONLY") {
            if let Some(path_without_prefix) = path.strip_prefix("axum-macros/") {
                path = path_without_prefix.to_owned();
            }

            if !path.contains(&format!("/{directory}/")) {
                return;
            }

            if path.contains("/fail/") {
                t.compile_fail(path);
            } else if path.contains("/pass/") {
                t.pass(path);
            } else {
                panic!()
            }
        } else {
            t.compile_fail(format!("tests/{directory}/fail/*.rs"));
            t.pass(format!("tests/{directory}/pass/*.rs"));
        }
    }

    #[rustversion::not(nightly)]
    fn go(_directory: &str) {}

    go(directory);
}
