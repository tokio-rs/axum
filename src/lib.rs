//! axum is a web application framework that focuses on ergonomics and modularity.
//!
//! # Table of contents
//!
//! - [High level features](#high-level-features)
//! - [Compatibility](#compatibility)
//! - [Handlers](#handlers)
//! - [Routing](#routing)
//!     - [Precedence](#precedence)
//!     - [Matching multiple methods](#matching-multiple-methods)
//! - [Extractors](#extractors)
//! - [Building responses](#building-responses)
//! - [Applying middleware](#applying-middleware)
//!     - [To individual handlers](#to-individual-handlers)
//!     - [To groups of routes](#to-groups-of-routes)
//!     - [Error handling](#error-handling)
//! - [Sharing state with handlers](#sharing-state-with-handlers)
//! - [Routing to any `Service`](#routing-to-any-service)
//! - [Nesting applications](#nesting-applications)
//! - [Required dependencies](#required-dependencies)
//! - [Examples](#examples)
//! - [Feature flags](#feature-flags)
//!
//! # High level features
//!
//! - Route requests to handlers with a macro free API.
//! - Declaratively parse requests using extractors.
//! - Simple and predictable error handling model.
//! - Generate responses with minimal boilerplate.
//! - Take full advantage of the [`tower`] and [`tower-http`] ecosystem of
//!   middleware, services, and utilities.
//!
//! In particular the last point is what sets `axum` apart from other frameworks.
//! `axum` doesn't have its own middleware system but instead uses
//! [`tower::Service`]. This means `axum` gets timeouts, tracing, compression,
//! authorization, and more, for free. It also enables you to share middleware with
//! applications written using [`hyper`] or [`tonic`].
//!
//! # Compatibility
//!
//! axum is designed to work with [tokio] and [hyper]. Runtime and
//! transport layer independence is not a goal, at least for the time being.
//!
//! # Example
//!
//! The "Hello, World!" of axum is:
//!
//! ```rust,no_run
//! use axum::prelude::*;
//!
//! #[tokio::main]
//! async fn main() {
//!     // build our application with a single route
//!     let app = route("/", get(|| async { "Hello, World!" }));
//!
//!     // run it with hyper on localhost:3000
//!     axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
//!         .serve(app.into_make_service())
//!         .await
//!         .unwrap();
//! }
//! ```
//!
//! # Handlers
//!
//! In axum a "handler" is an async function that accepts zero or more
//! ["extractors"](#extractors) as arguments and returns something that
//! can be converted [into a response](#building-responses).
//!
//! Handlers is where your custom domain logic lives and axum applications are
//! built by routing between handlers.
//!
//! Some examples of handlers:
//!
//! ```rust
//! use axum::prelude::*;
//! use bytes::Bytes;
//! use http::StatusCode;
//!
//! // Handler that immediately returns an empty `200 OK` response.
//! async fn unit_handler() {}
//!
//! // Handler that immediately returns an empty `200 OK` response with a plain
//! // text body.
//! async fn string_handler() -> String {
//!     "Hello, World!".to_string()
//! }
//!
//! // Handler that buffers the request body and returns it.
//! async fn echo(body: Bytes) -> Result<String, StatusCode> {
//!     if let Ok(string) = String::from_utf8(body.to_vec()) {
//!         Ok(string)
//!     } else {
//!         Err(StatusCode::BAD_REQUEST)
//!     }
//! }
//! ```
//!
//! # Routing
//!
//! Routing between handlers looks like this:
//!
//! ```rust,no_run
//! use axum::prelude::*;
//!
//! let app = route("/", get(get_slash).post(post_slash))
//!     .route("/foo", get(get_foo));
//!
//! async fn get_slash() {
//!     // `GET /` called
//! }
//!
//! async fn post_slash() {
//!     // `POST /` called
//! }
//!
//! async fn get_foo() {
//!     // `GET /foo` called
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! Routes can also be dynamic like `/users/:id`. See [extractors](#extractors)
//! for more details.
//!
//! You can also define routes separately and merge them with [`RoutingDsl::or`].
//!
//! ## Precedence
//!
//! Note that routes are matched _bottom to top_ so routes that should have
//! higher precedence should be added _after_ routes with lower precedence:
//!
//! ```rust
//! use axum::{prelude::*, body::BoxBody};
//! use tower::{Service, ServiceExt};
//! use http::{Method, Response, StatusCode};
//! use std::convert::Infallible;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // `/foo` also matches `/:key` so adding the routes in this order means `/foo`
//! // will be inaccessible.
//! let mut app = route("/foo", get(|| async { "/foo called" }))
//!     .route("/:key", get(|| async { "/:key called" }));
//!
//! // Even though we use `/foo` as the request URI, `/:key` takes precedence
//! // since its defined last.
//! let (status, body) = call_service(&mut app, Method::GET, "/foo").await;
//! assert_eq!(status, StatusCode::OK);
//! assert_eq!(body, "/:key called");
//!
//! // We have to add `/foo` after `/:key` since routes are matched bottom to
//! // top.
//! let mut new_app = route("/:key", get(|| async { "/:key called" }))
//!     .route("/foo", get(|| async { "/foo called" }));
//!
//! // Now it works
//! let (status, body) = call_service(&mut new_app, Method::GET, "/foo").await;
//! assert_eq!(status, StatusCode::OK);
//! assert_eq!(body, "/foo called");
//!
//! // And the other route works as well
//! let (status, body) = call_service(&mut new_app, Method::GET, "/bar").await;
//! assert_eq!(status, StatusCode::OK);
//! assert_eq!(body, "/:key called");
//!
//! // Little helper function to make calling a service easier. Just for
//! // demonstration purposes.
//! async fn call_service<S>(
//!     svc: &mut S,
//!     method: Method,
//!     uri: &str,
//! ) -> (StatusCode, String)
//! where
//!     S: Service<Request<Body>, Response = Response<BoxBody>, Error = Infallible>
//! {
//!     let req = Request::builder().method(method).uri(uri).body(Body::empty()).unwrap();
//!     let res = svc.ready().await.unwrap().call(req).await.unwrap();
//!
//!     let status = res.status();
//!
//!     let body = res.into_body();
//!     let body = hyper::body::to_bytes(body).await.unwrap();
//!     let body = String::from_utf8(body.to_vec()).unwrap();
//!
//!     (status, body)
//! }
//! # }
//! ```
//!
//! ## Matching multiple methods
//!
//! If you want a path to accept multiple HTTP methods you must add them all at
//! once:
//!
//! ```rust,no_run
//! use axum::prelude::*;
//!
//! // `GET /` and `POST /` are both accepted
//! let app = route("/", get(handler).post(handler));
//!
//! // This will _not_ work. Only `POST /` will be accessible.
//! let wont_work = route("/", get(handler)).route("/", post(handler));
//!
//! async fn handler() {}
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # axum::Server::bind(&"".parse().unwrap()).serve(wont_work.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! # Extractors
//!
//! An extractor is a type that implements [`FromRequest`]. Extractors is how
//! you pick apart the incoming request to get the parts your handler needs.
//!
//! For example, [`extract::Json`] is an extractor that consumes the request
//! body and deserializes it as JSON into some target type:
//!
//! ```rust,no_run
//! use axum::prelude::*;
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
//! async fn create_user(payload: extract::Json<CreateUser>) {
//!     let payload: CreateUser = payload.0;
//!
//!     // ...
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! [`extract::Path`] can be used to extract params from a dynamic URL. It
//! is compatible with any type that implements [`serde::Deserialize`], such as
//! [`Uuid`]:
//!
//! ```rust,no_run
//! use axum::prelude::*;
//! use uuid::Uuid;
//!
//! let app = route("/users/:id", post(create_user));
//!
//! async fn create_user(extract::Path(user_id): extract::Path<Uuid>) {
//!     // ...
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! You can also apply multiple extractors:
//!
//! ```rust,no_run
//! use axum::prelude::*;
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
//!     extract::Path(user_id): extract::Path<Uuid>,
//!     pagination: Option<extract::Query<Pagination>>,
//! ) {
//!     let pagination: Pagination = pagination.unwrap_or_default().0;
//!
//!     // ...
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! Additionally `Request<Body>` is itself an extractor:
//!
//! ```rust,no_run
//! use axum::prelude::*;
//!
//! let app = route("/users/:id", post(handler));
//!
//! async fn handler(req: Request<Body>) {
//!     // ...
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! However it cannot be combined with other extractors since it consumes the
//! entire request.
//!
//! See the [`extract`] module for more details.
//!
//! [`Uuid`]: https://docs.rs/uuid/latest/uuid/
//! [`FromRequest`]: crate::extract::FromRequest
//!
//! # Building responses
//!
//! Anything that implements [`IntoResponse`](response::IntoResponse) can be
//! returned from a handler:
//!
//! ```rust,no_run
//! use axum::{body::Body, response::{Html, Json}, prelude::*};
//! use http::{StatusCode, Response, Uri};
//! use serde_json::{Value, json};
//!
//! // We've already seen returning &'static str
//! async fn plain_text() -> &'static str {
//!     "foo"
//! }
//!
//! // String works too and will get a `text/plain` content-type
//! async fn plain_text_string(uri: Uri) -> String {
//!     format!("Hi from {}", uri.path())
//! }
//!
//! // Bytes will get a `application/octet-stream` content-type
//! async fn bytes() -> Vec<u8> {
//!     vec![1, 2, 3, 4]
//! }
//!
//! // `()` gives an empty response
//! async fn empty() {}
//!
//! // `StatusCode` gives an empty response with that status code
//! async fn empty_with_status() -> StatusCode {
//!     StatusCode::NOT_FOUND
//! }
//!
//! // A tuple of `StatusCode` and something that implements `IntoResponse` can
//! // be used to override the status code
//! async fn with_status() -> (StatusCode, &'static str) {
//!     (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong")
//! }
//!
//! // `Html` gives a content-type of `text/html`
//! async fn html() -> Html<&'static str> {
//!     Html("<h1>Hello, World!</h1>")
//! }
//!
//! // `Json` gives a content-type of `application/json` and works with any type
//! // that implements `serde::Serialize`
//! async fn json() -> Json<Value> {
//!     Json(json!({ "data": 42 }))
//! }
//!
//! // `Result<T, E>` where `T` and `E` implement `IntoResponse` is useful for
//! // returning errors
//! async fn result() -> Result<&'static str, StatusCode> {
//!     Ok("all good")
//! }
//!
//! // `Response` gives full control
//! async fn response() -> Response<Body> {
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
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! # Applying middleware
//!
//! axum is designed to take full advantage of the tower and tower-http
//! ecosystem of middleware:
//!
//! ## To individual handlers
//!
//! A middleware can be applied to a single handler like so:
//!
//! ```rust,no_run
//! use axum::prelude::*;
//! use tower::limit::ConcurrencyLimitLayer;
//!
//! let app = route(
//!     "/",
//!     get(handler.layer(ConcurrencyLimitLayer::new(100))),
//! );
//!
//! async fn handler() {}
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! ## To groups of routes
//!
//! Middleware can also be applied to a group of routes like so:
//!
//! ```rust,no_run
//! use axum::prelude::*;
//! use tower::limit::ConcurrencyLimitLayer;
//!
//! let app = route("/", get(get_slash))
//!     .route("/foo", post(post_foo))
//!     .layer(ConcurrencyLimitLayer::new(100));
//!
//! async fn get_slash() {}
//!
//! async fn post_foo() {}
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! ## Error handling
//!
//! Handlers created from async functions must always produce a response, even
//! when returning a `Result<T, E>` the error type must implement
//! [`IntoResponse`]. In practice this makes error handling very predictable and
//! easier to reason about.
//!
//! However when applying middleware, or embedding other tower services, errors
//! might happen. For example [`Timeout`] will return an error if the timeout
//! elapses. By default these errors will be propagated all the way up to hyper
//! where the connection will be closed. If that isn't desirable you can call
//! [`handle_error`](handler::Layered::handle_error) to handle errors from
//! adding a middleware to a handler:
//!
//! ```rust,no_run
//! use axum::prelude::*;
//! use tower::{
//!     BoxError, timeout::{TimeoutLayer, error::Elapsed},
//! };
//! use std::{borrow::Cow, time::Duration, convert::Infallible};
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
//!                 return Ok::<_, Infallible>((
//!                     StatusCode::REQUEST_TIMEOUT,
//!                     "Request took too long".into(),
//!                 ));
//!             }
//!
//!             // If we encounter some error we don't handle return a generic
//!             // error
//!             return Ok::<_, Infallible>((
//!                 StatusCode::INTERNAL_SERVER_ERROR,
//!                 // `Cow` lets us return either `&str` or `String`
//!                 Cow::from(format!("Unhandled internal error: {}", error)),
//!             ));
//!         })),
//! );
//!
//! async fn handle() {}
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! The closure passed to [`handle_error`](handler::Layered::handle_error) must
//! return `Result<T, E>` where `T` implements
//! [`IntoResponse`](response::IntoResponse).
//!
//! See [`routing::RoutingDsl::handle_error`] for more details.
//!
//! ## Applying multiple middleware
//!
//! [`tower::ServiceBuilder`] can be used to combine multiple middleware:
//!
//! ```rust,no_run
//! use axum::prelude::*;
//! use tower::ServiceBuilder;
//! use tower_http::compression::CompressionLayer;
//! use std::{borrow::Cow, time::Duration};
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
//!     .layer(middleware_stack);
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
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
//! use axum::{AddExtensionLayer, prelude::*};
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
//!     state: extract::Extension<Arc<State>>,
//! ) {
//!     let state: Arc<State> = state.0;
//!
//!     // ...
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! # Routing to any [`Service`]
//!
//! axum also supports routing to general [`Service`]s:
//!
//! ```rust,no_run
//! use axum::{service, prelude::*};
//! use tower_http::services::ServeFile;
//! use http::Response;
//! use std::convert::Infallible;
//! use tower::service_fn;
//!
//! let app = route(
//!     // Any request to `/` goes to a service
//!     "/",
//!     // Services who's response body is not `axum::body::BoxBody`
//!     // can be wrapped in `axum::service::any` (or one of the other routing filters)
//!     // to have the response body mapped
//!     service::any(service_fn(|_: Request<Body>| async {
//!         let res = Response::new(Body::from("Hi from `GET /`"));
//!         Ok(res)
//!     }))
//! ).route(
//!     "/foo",
//!     // This service's response body is `axum::body::BoxBody` so
//!     // it can be routed to directly.
//!     service_fn(|req: Request<Body>| async move {
//!         let body = Body::from(format!("Hi from `{} /foo`", req.method()));
//!         let body = axum::body::box_body(body);
//!         let res = Response::new(body);
//!         Ok(res)
//!     })
//! ).route(
//!     // GET `/static/Cargo.toml` goes to a service from tower-http
//!     "/static/Cargo.toml",
//!     service::get(ServeFile::new("Cargo.toml"))
//! );
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! Routing to arbitrary services in this way has complications for backpressure
//! ([`Service::poll_ready`]). See the [`service`] module for more details.
//!
//! # Nesting applications
//!
//! Applications can be nested by calling [`nest`](routing::nest):
//!
//! ```rust,no_run
//! use axum::{prelude::*, routing::BoxRoute, body::{Body, BoxBody}};
//! use tower_http::services::ServeFile;
//! use http::Response;
//!
//! fn api_routes() -> BoxRoute<Body> {
//!     route("/users", get(|_: Request<Body>| async { /* ... */ })).boxed()
//! }
//!
//! let app = route("/", get(|_: Request<Body>| async { /* ... */ }))
//!     .nest("/api", api_routes());
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! # Required dependencies
//!
//! To use axum there are a few dependencies you have pull in as well:
//!
//! ```toml
//! [dependencies]
//! axum = "<latest-version>"
//! hyper = { version = "<latest-version>", features = ["full"] }
//! tokio = { version = "<latest-version>", features = ["full"] }
//! tower = "<latest-version>"
//! ```
//!
//! The `"full"` feature for hyper and tokio isn't strictly necessary but its
//! the easiest way to get started.
//!
//! Note that [`axum::Server`] is re-exported by axum so if thats all you need
//! then you don't have to explicitly depend on hyper.
//!
//! Tower isn't strictly necessary either but helpful for testing. See the
//! testing example in the repo to learn more about testing axum apps.
//!
//! # Examples
//!
//! The axum repo contains [a number of examples][examples] that show how to put all the
//! pieces together.
//!
//! # Feature flags
//!
//! axum uses a set of [feature flags] to reduce the amount of compiled and
//! optional dependencies.
//!
//! The following optional features are available:
//!
//! - `ws`: Enables WebSockets support.
//! - `headers`: Enables extracting typed headers via [`extract::TypedHeader`].
//! - `multipart`: Enables parsing `multipart/form-data` requests with [`extract::Multipart`].
//!
//! [`tower`]: https://crates.io/crates/tower
//! [`tower-http`]: https://crates.io/crates/tower-http
//! [`tokio`]: http://crates.io/crates/tokio
//! [`hyper`]: http://crates.io/crates/hyper
//! [`tonic`]: http://crates.io/crates/tonic
//! [feature flags]: https://doc.rust-lang.org/cargo/reference/features.html#the-features-section
//! [`IntoResponse`]: crate::response::IntoResponse
//! [`Timeout`]: tower::timeout::Timeout
//! [examples]: https://github.com/tokio-rs/axum/tree/main/examples
//! [`RoutingDsl::or`]: crate::routing::RoutingDsl::or
//! [`axum::Server`]: hyper::server::Server

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

use http::Request;
use routing::{EmptyRouter, Route};
use tower::Service;

#[macro_use]
pub(crate) mod macros;

mod buffer;
mod error;
mod json;
mod util;

pub mod body;
pub mod extract;
pub mod handler;
pub mod response;
pub mod routing;
pub mod service;
pub mod sse;

#[cfg(test)]
mod tests;

#[doc(no_inline)]
pub use async_trait::async_trait;
#[doc(no_inline)]
pub use http;
#[doc(no_inline)]
pub use hyper::Server;
#[doc(no_inline)]
pub use tower_http::add_extension::{AddExtension, AddExtensionLayer};

pub use self::{error::Error, json::Json};

pub mod prelude {
    //! Re-exports of important traits, types, and functions used with axum. Meant to be glob
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
/// # Examples
///
/// ```rust
/// use axum::prelude::*;
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
pub fn route<S, B>(description: &str, service: S) -> Route<S, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    use routing::RoutingDsl;

    routing::EmptyRouter::not_found().route(description, service)
}

mod sealed {
    #![allow(unreachable_pub, missing_docs)]

    pub trait Sealed {}
}
