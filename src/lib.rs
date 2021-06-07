//! tower-web (name pending) is a tiny web application framework that focuses on
//! ergonomics and modularity.
//!
//! ## Goals
//!
//! - Ease of use. Building web apps in Rust should be as easy as `async fn
//! handle(Request) -> Response`.
//! - Solid foundation. tower-web is built on top of tower and makes it easy to
//! plug in any middleware from the [tower] and [tower-http] ecosystem.
//! - Focus on routing, extracting data from requests, and generating responses.
//! tower middleware can handle the rest.
//! - Macro free core. Macro frameworks have their place but tower-web focuses
//! on providing a core that is macro free.
//!
//! # Example
//!
//! The "Hello, World!" of tower-web is:
//!
//! ```rust,no_run
//! use tower_web::prelude::*;
//! use hyper::Server;
//! use std::net::SocketAddr;
//! use tower::make::Shared;
//!
//! #[tokio::main]
//! async fn main() {
//!     // build our application with a single route
//!     let app = route("/", get(|request: Request<Body>| async {
//!         "Hello, World!"
//!     }));
//!
//!     // run it with hyper on localhost:3000
//!     let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
//!     let server = Server::bind(&addr).serve(Shared::new(app));
//!     server.await.unwrap();
//! }
//! ```
//!
//! # Routing
//!
//! Routing between handlers looks like this:
//!
//! ```rust,no_run
//! use tower_web::prelude::*;
//!
//! let app = route("/", get(get_slash).post(post_slash))
//!     .route("/foo", get(get_foo));
//!
//! async fn get_slash(req: Request<Body>) {
//!     // `GET /` called
//! }
//!
//! async fn post_slash(req: Request<Body>) {
//!     // `POST /` called
//! }
//!
//! async fn get_foo(req: Request<Body>) {
//!     // `GET /foo` called
//! }
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! Routes can also be dynamic like `/users/:id`. See ["Extracting data from
//! requests"](#extracting-data-from-requests) for more details on that.
//!
//! # Responses
//!
//! Anything that implements [`IntoResponse`] can be returned from a handler:
//!
//! ```rust,no_run
//! use tower_web::{body::Body, response::{Html, Json}, prelude::*};
//! use http::{StatusCode, Response};
//! use serde_json::{Value, json};
//!
//! // We've already seen returning &'static str
//! async fn plain_text(req: Request<Body>) -> &'static str {
//!     "foo"
//! }
//!
//! // String works too and will get a text/plain content-type
//! async fn plain_text_string(req: Request<Body>) -> String {
//!     format!("Hi from {}", req.uri().path())
//! }
//!
//! // Bytes will get a `application/octet-stream` content-type
//! async fn bytes(req: Request<Body>) -> Vec<u8> {
//!     vec![1, 2, 3, 4]
//! }
//!
//! // `()` gives an empty response
//! async fn empty(req: Request<Body>) {}
//!
//! // `StatusCode` gives an empty response with that status code
//! async fn empty_with_status(req: Request<Body>) -> StatusCode {
//!     StatusCode::NOT_FOUND
//! }
//!
//! // A tuple of `StatusCode` and something that implements `IntoResponse` can
//! // be used to override the status code
//! async fn with_status(req: Request<Body>) -> (StatusCode, &'static str) {
//!     (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong")
//! }
//!
//! // `Html` gives a content-type of `text/html`
//! async fn html(req: Request<Body>) -> Html<&'static str> {
//!     Html("<h1>Hello, World!</h1>")
//! }
//!
//! // `Json` gives a content-type of `application/json` and works with my type
//! // that implements `serde::Serialize`
//! async fn json(req: Request<Body>) -> Json<Value> {
//!     Json(json!({ "data": 42 }))
//! }
//!
//! // `Result<T, E>` where `T` and `E` implement `IntoResponse` is useful for
//! // returning errors
//! async fn result(req: Request<Body>) -> Result<&'static str, StatusCode> {
//!     Ok("all good")
//! }
//!
//! // `Response` gives full control
//! async fn response(req: Request<Body>) -> Response<Body> {
//!     Response::builder().body(Body::empty()).unwrap()
//! }
//!
//! let app = route("/plain_text", get(plain_text))
//!     .route("/plain_text_string", get(plain_text_string))
//!     .route("/bytes", get(bytes))
//!     .route("/empty", get(empty))
//!     .route("/empty_with_status", get(empty_with_status))
//!     .route("/with_status", get(with_status))
//!     .route("/html", get(html))
//!     .route("/json", get(json))
//!     .route("/result", get(result))
//!     .route("/response", get(response));
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! See the [`response`] module for more details.
//!
//! # Extracting data from requests
//!
//! A handler function must always take `Request<Body>` as its first argument
//! but any arguments following are called "extractors". Any type that
//! implements [`FromRequest`](crate::extract::FromRequest) can be used as an
//! extractor.
//!
//! [`extract::Json`] is an extractor that consumes the request body and
//! deserializes as as JSON into some target type:
//!
//! ```rust,no_run
//! use tower_web::prelude::*;
//! use serde::Deserialize;
//!
//! let app = route("/users", post(create_user));
//!
//! #[derive(Deserialize)]
//! struct CreateUser {
//!     email: String,
//!     password: String,
//! }
//!
//! async fn create_user(req: Request<Body>, payload: extract::Json<CreateUser>) {
//!     let payload: CreateUser = payload.0;
//!
//!     // ...
//! }
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! [`extract::UrlParams`] can be used to extract params from a dynamic URL. It
//! is compatible with any type that implements [`std::str::FromStr`], such as
//! [`Uuid`]:
//!
//! ```rust,no_run
//! use tower_web::prelude::*;
//! use uuid::Uuid;
//!
//! let app = route("/users/:id", post(create_user));
//!
//! async fn create_user(req: Request<Body>, params: extract::UrlParams<(Uuid,)>) {
//!     let (user_id,) = params.0;
//!
//!     // ...
//! }
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! There is also [`UrlParamsMap`](extract::UrlParamsMap) which provide a map
//! like API for extracting URL params.
//!
//! You can also apply multiple extractors:
//!
//! ```rust,no_run
//! use tower_web::prelude::*;
//! use uuid::Uuid;
//! use serde::Deserialize;
//!
//! let app = route("/users/:id/things", get(get_user_things));
//!
//! #[derive(Deserialize)]
//! struct Pagination {
//!     page: usize,
//!     per_page: usize,
//! }
//!
//! impl Default for Pagination {
//!     fn default() -> Self {
//!         Self { page: 1, per_page: 30 }
//!     }
//! }
//!
//! async fn get_user_things(
//!     req: Request<Body>,
//!     params: extract::UrlParams<(Uuid,)>,
//!     pagination: Option<extract::Query<Pagination>>,
//! ) {
//!     let user_id: Uuid = (params.0).0;
//!     let pagination: Pagination = pagination.unwrap_or_default().0;
//!
//!     // ...
//! }
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! See the [`extract`] module for more details.
//!
//! [`Uuid`]: https://docs.rs/uuid/latest/uuid/
//!
//! # Applying middleware
//!
//! tower-web is designed to take full advantage of the tower and tower-http
//! ecosystem of middleware:
//!
//! ## To individual handlers
//!
//! A middleware can be applied to a single handler like so:
//!
//! ```rust,no_run
//! use tower_web::prelude::*;
//! use tower::limit::ConcurrencyLimitLayer;
//!
//! let app = route(
//!     "/",
//!     get(handler.layer(ConcurrencyLimitLayer::new(100))),
//! );
//!
//! async fn handler(req: Request<Body>) {}
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! ## To groups of routes
//!
//! Middleware can also be applied to a group of routes like so:
//!
//! ```rust,no_run
//! use tower_web::prelude::*;
//! use tower::limit::ConcurrencyLimitLayer;
//!
//! let app = route("/", get(get_slash))
//!     .route("/foo", post(post_foo))
//!     .layer(ConcurrencyLimitLayer::new(100));
//!
//! async fn get_slash(req: Request<Body>) {}
//!
//! async fn post_foo(req: Request<Body>) {}
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! ## Error handling
//!
//! tower-web requires all errors to be handled. That is done by using
//! [`std::convert::Infallible`] as the error type in all its [`Service`]
//! implementations.
//!
//! For handlers created from async functions this is works automatically since
//! handlers must return something that implements [`IntoResponse`], even if its
//! a `Result`.
//!
//! However middleware might add new failure cases that has to be handled. For
//! that tower-web provides a `handle_error` combinator:
//!
//! ```rust,no_run
//! use tower_web::prelude::*;
//! use tower::{
//!     BoxError, timeout::{TimeoutLayer, error::Elapsed},
//! };
//! use std::{borrow::Cow, time::Duration};
//! use http::StatusCode;
//!
//! let app = route(
//!     "/",
//!     get(handle
//!         .layer(TimeoutLayer::new(Duration::from_secs(30)))
//!         // `Timeout` uses `BoxError` as the error type
//!         .handle_error(|error: BoxError| {
//!             // Check if the actual error type is `Elapsed` which
//!             // `Timeout` returns
//!             if error.is::<Elapsed>() {
//!                 return (StatusCode::REQUEST_TIMEOUT, "Request took too long".into());
//!             }
//!
//!             // If we encounter some error we don't handle return a generic
//!             // error
//!             return (
//!                 StatusCode::INTERNAL_SERVER_ERROR,
//!                 // `Cow` lets us return either `&str` or `String`
//!                 Cow::from(format!("Unhandled internal error: {}", error)),
//!             );
//!         })),
//! );
//!
//! async fn handle(req: Request<Body>) {}
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! The closure passed to `handle_error` must return something that implements
//! `IntoResponse`.
//!
//! `handle_error` is also available on a group of routes with middleware
//! applied:
//!
//! ```rust,no_run
//! use tower_web::prelude::*;
//! use tower::{
//!     BoxError, timeout::{TimeoutLayer, error::Elapsed},
//! };
//! use std::{borrow::Cow, time::Duration};
//! use http::StatusCode;
//!
//! let app = route("/", get(handle))
//!     .layer(TimeoutLayer::new(Duration::from_secs(30)))
//!     .handle_error(|error: BoxError| {
//!         // ...
//!     });
//!
//! async fn handle(req: Request<Body>) {}
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! ## Applying multiple middleware
//!
//! [`tower::ServiceBuilder`] can be used to combine multiple middleware:
//!
//! ```rust,no_run
//! use tower_web::prelude::*;
//! use tower::{
//!     ServiceBuilder, BoxError,
//!     load_shed::error::Overloaded,
//!     timeout::error::Elapsed,
//! };
//! use tower_http::compression::CompressionLayer;
//! use std::{borrow::Cow, time::Duration};
//! use http::StatusCode;
//!
//! let middleware_stack = ServiceBuilder::new()
//!     // Return an error after 30 seconds
//!     .timeout(Duration::from_secs(30))
//!     // Shed load if we're receiving too many requests
//!     .load_shed()
//!     // Process at most 100 requests concurrently
//!     .concurrency_limit(100)
//!     // Compress response bodies
//!     .layer(CompressionLayer::new())
//!     .into_inner();
//!
//! let app = route("/", get(|_: Request<Body>| async { /* ... */ }))
//!     .layer(middleware_stack)
//!     .handle_error(|error: BoxError| {
//!         if error.is::<Overloaded>() {
//!             return (
//!                 StatusCode::SERVICE_UNAVAILABLE,
//!                 "Try again later".into(),
//!             );
//!         }
//!
//!         if error.is::<Elapsed>() {
//!             return (
//!                 StatusCode::REQUEST_TIMEOUT,
//!                 "Request took too long".into(),
//!             );
//!         };
//!
//!         return (
//!             StatusCode::INTERNAL_SERVER_ERROR,
//!             Cow::from(format!("Unhandled internal error: {}", error)),
//!         );
//!     });
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! # Sharing state with handlers
//!
//! It is common to share some state between handlers for example to share a
//! pool of database connections or clients to other services. That can be done
//! using the [`AddExtension`] middleware (applied with [`AddExtensionLayer`])
//! and the [`extract::Extension`] extractor:
//!
//! ```rust,no_run
//! use tower_web::{AddExtensionLayer, prelude::*};
//! use std::sync::Arc;
//!
//! struct State {
//!     // ...
//! }
//!
//! let shared_state = Arc::new(State { /* ... */ });
//!
//! let app = route("/", get(handler)).layer(AddExtensionLayer::new(shared_state));
//!
//! async fn handler(
//!     req: Request<Body>,
//!     state: extract::Extension<Arc<State>>,
//! ) {
//!     let state: Arc<State> = state.0;
//!
//!     // ...
//! }
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! # Routing to any [`Service`]
//!
//! tower-web also supports routing to general [`Service`]s:
//!
//! ```rust,no_run
//! use tower_web::{
//!     service, prelude::*,
//!     // `ServiceExt` adds `handle_error` to any `Service`
//!     ServiceExt,
//! };
//! use tower_http::services::ServeFile;
//! use http::Response;
//! use std::convert::Infallible;
//! use tower::{service_fn, BoxError};
//!
//! let app = route(
//!     // Any request to `/` goes to a service
//!     "/",
//!     service_fn(|_: Request<Body>| async {
//!         let res = Response::new(Body::from("Hi from `GET /`"));
//!         Ok::<_, Infallible>(res)
//!     })
//! ).route(
//!     // GET `/static/Cargo.toml` goes to a service from tower-http
//!     "/static/Cargo.toml",
//!     service::get(
//!         ServeFile::new("Cargo.toml")
//!             // Errors must be handled
//!             .handle_error(|error: std::io::Error| { /* ... */ })
//!     )
//! );
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! See the [`service`] module for more details.
//!
//! # Nesting applications
//!
//! Applications can be nested by calling `nest`:
//!
//! ```rust,no_run
//! use tower_web::{prelude::*, routing::BoxRoute, body::BoxBody};
//! use tower_http::services::ServeFile;
//! use http::Response;
//! use std::convert::Infallible;
//!
//! fn api_routes() -> BoxRoute<BoxBody> {
//!     route("/users", get(|_: Request<Body>| async { /* ... */ })).boxed()
//! }
//!
//! let app = route("/", get(|_: Request<Body>| async { /* ... */ }))
//!     .nest("/api", api_routes());
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! `nest` can also be used to serve static files from a directory:
//!
//! ```rust,no_run
//! use tower_web::{prelude::*, ServiceExt, routing::nest};
//! use tower_http::services::ServeDir;
//! use http::Response;
//! use std::convert::Infallible;
//! use tower::{service_fn, BoxError};
//!
//! let app = nest(
//!     "/images",
//!     ServeDir::new("public/images").handle_error(|error: std::io::Error| {
//!         // ...
//!     })
//! );
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
//! # };
//! ```
//!
//! [tower]: https://crates.io/crates/tower
//! [tower-http]: https://crates.io/crates/tower-http

// #![doc(html_root_url = "https://docs.rs/tower-http/0.1.0")]
#![warn(
    clippy::all,
    clippy::dbg_macro,
    clippy::todo,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::pub_enum_variant_names,
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
#![deny(unreachable_pub, broken_intra_doc_links, private_in_public)]
#![allow(
    elided_lifetimes_in_paths,
    // TODO: Remove this once the MSRV bumps to 1.42.0 or above.
    clippy::match_like_matches_macro,
    clippy::type_complexity
)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]

use self::body::Body;
use bytes::Bytes;
use http::{Request, Response};
use response::IntoResponse;
use routing::{EmptyRouter, Route};
use std::convert::Infallible;
use tower::{BoxError, Service};

#[macro_use]
pub(crate) mod macros;

pub mod body;
pub mod extract;
pub mod handler;
pub mod response;
pub mod routing;
pub mod service;

#[cfg(test)]
mod tests;

pub use async_trait::async_trait;
pub use tower_http::add_extension::{AddExtension, AddExtensionLayer};

pub mod prelude {
    //! Re-exports of important traits, types, and functions used with tower-web. Meant to be glob
    //! imported.

    pub use crate::body::Body;
    pub use crate::extract;
    pub use crate::handler::{
        any, connect, delete, get, head, options, patch, post, put, trace, Handler,
    };
    pub use crate::response;
    pub use crate::route;
    pub use crate::routing::RoutingDsl;
    pub use http::Request;
}

/// Create a route.
///
/// `description` is a string of path segments separated by `/`. Each segment
/// can be either concrete or a capture:
///
/// - `/foo/bar/baz` will only match requests where the path is `/foo/bar/bar`.
/// - `/:foo` will match any route with exactly one segment _and_ it will
/// capture the first segment and store it at the key `foo`.
///
/// `service` is the [`Service`] that should receive the request if the path
/// matches `description`.
///
/// Note that `service`'s error type must be [`Infallible`] meaning you must
/// handle all errors. If you're creating handlers from async functions that is
/// handled automatically but if you're routing to some other [`Service`] you
/// might need to use [`handle_error`](ServiceExt::handle_error) to map errors
/// into responses.
///
/// # Examples
///
/// ```rust
/// use tower_web::prelude::*;
/// # use std::convert::Infallible;
/// # use http::Response;
/// # let service = tower::service_fn(|_: Request<Body>| async {
/// #     Ok::<Response<Body>, Infallible>(Response::new(Body::empty()))
/// # });
///
/// route("/", service);
/// route("/users", service);
/// route("/users/:id", service);
/// route("/api/:version/users/:id/action", service);
/// ```
///
/// # Panics
///
/// Panics if `description` doesn't start with `/`.
pub fn route<S>(description: &str, service: S) -> Route<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    use routing::RoutingDsl;

    routing::EmptyRouter.route(description, service)
}

/// Extension trait that adds additional methods to [`Service`].
pub trait ServiceExt<B>: Service<Request<Body>, Response = Response<B>> {
    /// Handle errors from a service.
    ///
    /// tower-web requires all handles to never return errors. If you route to
    /// [`Service`], not created by tower-web, who's error isn't `Infallible`
    /// you can use this combinator to handle the error.
    ///
    /// `handle_error` takes a closure that will map errors from the service
    /// into responses. The closure's return type must implement
    /// [`IntoResponse`].
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use tower_web::{
    ///     service, prelude::*,
    ///     ServiceExt,
    /// };
    /// use http::Response;
    /// use tower::{service_fn, BoxError};
    ///
    /// // A service that might fail with `std::io::Error`
    /// let service = service_fn(|_: Request<Body>| async {
    ///     let res = Response::new(Body::empty());
    ///     Ok::<_, std::io::Error>(res)
    /// });
    ///
    /// let app = route(
    ///     "/",
    ///     service.handle_error(|error: std::io::Error| {
    ///         // Handle error by returning something that implements `IntoResponse`
    ///     }),
    /// );
    /// #
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap()).serve(tower::make::Shared::new(app)).await;
    /// # };
    /// ```
    fn handle_error<F, Res>(self, f: F) -> service::HandleError<Self, F>
    where
        Self: Sized,
        F: FnOnce(Self::Error) -> Res,
        Res: IntoResponse,
        B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
    {
        service::HandleError::new(self, f)
    }
}

impl<S, B> ServiceExt<B> for S where S: Service<Request<Body>, Response = Response<B>> {}

pub(crate) trait ResultExt<T> {
    fn unwrap_infallible(self) -> T;
}

impl<T> ResultExt<T> for Result<T, Infallible> {
    fn unwrap_infallible(self) -> T {
        match self {
            Ok(value) => value,
            Err(err) => match err {},
        }
    }
}
