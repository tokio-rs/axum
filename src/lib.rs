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
//!     - [Routing to any `Service`](#routing-to-any-service)
//!         - [Routing to fallible services](#routing-to-fallible-services)
//!     - [Nesting routes](#nesting-routes)
//! - [Extractors](#extractors)
//! - [Building responses](#building-responses)
//! - [Applying middleware](#applying-middleware)
//!     - [To individual handlers](#to-individual-handlers)
//!     - [To groups of routes](#to-groups-of-routes)
//!     - [Error handling](#error-handling)
//!     - [Applying multiple middleware](#applying-multiple-middleware)
//!     - [Commonly used middleware](#commonly-used-middleware)
//!     - [Writing your own middleware](#writing-your-own-middleware)
//! - [Sharing state with handlers](#sharing-state-with-handlers)
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
//! use axum::{
//!     handler::get,
//!     Router,
//! };
//!
//! #[tokio::main]
//! async fn main() {
//!     // build our application with a single route
//!     let app = Router::new().route("/", get(|| async { "Hello, World!" }));
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
//! use axum::{
//!     handler::get,
//!     Router,
//! };
//!
//! let app = Router::new()
//!     .route("/", get(get_slash).post(post_slash))
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
//! You can also define routes separately and merge them with [`Router::or`].
//!
//! ## Precedence
//!
//! Note that routes are matched _bottom to top_ so routes that should have
//! higher precedence should be added _after_ routes with lower precedence:
//!
//! ```rust
//! use axum::{
//!     body::{Body, BoxBody},
//!     handler::get,
//!     http::Request,
//!     Router,
//! };
//! use tower::{Service, ServiceExt};
//! use http::{Method, Response, StatusCode};
//! use std::convert::Infallible;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // `/foo` also matches `/:key` so adding the routes in this order means `/foo`
//! // will be inaccessible.
//! let mut app = Router::new()
//!     .route("/foo", get(|| async { "/foo called" }))
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
//! let mut new_app = Router::new()
//!     .route("/:key", get(|| async { "/:key called" }))
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
//! ## Routing to any [`Service`]
//!
//! axum also supports routing to general [`Service`]s:
//!
//! ```rust,no_run
//! use axum::{
//!     body::Body,
//!     http::Request,
//!     Router,
//!     service
//! };
//! use tower_http::services::ServeFile;
//! use http::Response;
//! use std::convert::Infallible;
//! use tower::service_fn;
//!
//! let app = Router::new()
//!     .route(
//!         // Any request to `/` goes to a service
//!         "/",
//!         // Services who's response body is not `axum::body::BoxBody`
//!         // can be wrapped in `axum::service::any` (or one of the other routing filters)
//!         // to have the response body mapped
//!         service::any(service_fn(|_: Request<Body>| async {
//!             let res = Response::new(Body::from("Hi from `GET /`"));
//!             Ok(res)
//!         }))
//!     )
//!     .route(
//!         "/foo",
//!         // This service's response body is `axum::body::BoxBody` so
//!         // it can be routed to directly.
//!         service_fn(|req: Request<Body>| async move {
//!             let body = Body::from(format!("Hi from `{} /foo`", req.method()));
//!             let body = axum::body::box_body(body);
//!             let res = Response::new(body);
//!             Ok(res)
//!         })
//!     )
//!     .route(
//!         // GET `/static/Cargo.toml` goes to a service from tower-http
//!         "/static/Cargo.toml",
//!         service::get(ServeFile::new("Cargo.toml"))
//!     );
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! Routing to arbitrary services in this way has complications for backpressure
//! ([`Service::poll_ready`]). See the [`service`] module for more details.
//!
//! ### Routing to fallible services
//!
//! Note that routing to general services has a small gotcha when it comes to
//! errors. axum currently does not support mixing routes to fallible services
//! with infallible handlers. For example this does _not_ compile:
//!
//! ```compile_fail
//! use axum::{
//!     Router,
//!     service,
//!     handler::get,
//!     http::{Request, Response},
//!     body::Body,
//! };
//! use std::io;
//! use tower::service_fn;
//!
//! let app = Router::new()
//!     // this route cannot fail
//!     .route("/foo", get(|| async {}))
//!     // this route can fail with io::Error
//!     .route(
//!         "/",
//!         service::get(service_fn(|_req: Request<Body>| async {
//!             let contents = tokio::fs::read_to_string("some_file").await?;
//!             Ok::<_, io::Error>(Response::new(Body::from(contents)))
//!         })),
//!     );
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! The solution is to use [`handle_error`] and handle the error from the
//! service:
//!
//! ```
//! use axum::{
//!     Router,
//!     service,
//!     handler::get,
//!     http::{Request, Response},
//!     response::IntoResponse,
//!     body::Body,
//! };
//! use std::{io, convert::Infallible};
//! use tower::service_fn;
//!
//! let app = Router::new()
//!     // this route cannot fail
//!     .route("/foo", get(|| async {}))
//!     // this route can fail with io::Error
//!     .route(
//!         "/",
//!         service::get(service_fn(|_req: Request<Body>| async {
//!             let contents = tokio::fs::read_to_string("some_file").await?;
//!             Ok::<_, io::Error>(Response::new(Body::from(contents)))
//!         }))
//!         .handle_error(handle_io_error),
//!     );
//!
//! fn handle_io_error(error: io::Error) -> Result<impl IntoResponse, Infallible> {
//!     # Ok(())
//!     // ...
//! }
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! In this particular case you can also handle the error directly in
//! `service_fn` but that is not possible, if you're routing to a service which
//! you don't control.
//!
//! See ["Error handling"](#error-handling) for more details on [`handle_error`]
//! and error handling in general.
//!
//! ## Nesting routes
//!
//! Routes can be nested by calling [`Router::nest`](routing::Router::nest):
//!
//! ```rust,no_run
//! use axum::{
//!     body::{Body, BoxBody},
//!     http::Request,
//!     handler::get,
//!     Router,
//!     routing::BoxRoute
//! };
//! use tower_http::services::ServeFile;
//! use http::Response;
//!
//! fn api_routes() -> Router<BoxRoute> {
//!     Router::new()
//!         .route("/users", get(|_: Request<Body>| async { /* ... */ }))
//!         .boxed()
//! }
//!
//! let app = Router::new()
//!     .route("/", get(|_: Request<Body>| async { /* ... */ }))
//!     .nest("/api", api_routes());
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! Note that nested routes will not see the orignal request URI but instead
//! have the matched prefix stripped. This is necessary for services like static
//! file serving to work. Use [`OriginalUri`] if you need the original request
//! URI.
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
//! use axum::{
//!     extract,
//!     handler::post,
//!     Router,
//! };
//! use serde::Deserialize;
//!
//! let app = Router::new().route("/users", post(create_user));
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
//! use axum::{
//!     extract,
//!     handler::post,
//!     Router,
//! };
//! use uuid::Uuid;
//!
//! let app = Router::new().route("/users/:id", post(create_user));
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
//! use axum::{
//!     extract,
//!     handler::get,
//!     Router,
//! };
//! use uuid::Uuid;
//! use serde::Deserialize;
//!
//! let app = Router::new().route("/users/:id/things", get(get_user_things));
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
//! use axum::{
//!     body::Body,
//!     handler::post,
//!     http::Request,
//!     Router,
//! };
//!
//! let app = Router::new().route("/users/:id", post(handler));
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
//! use axum::{
//!     body::Body,
//!     handler::{get, Handler},
//!     http::{Request, header::{HeaderMap, HeaderName, HeaderValue}},
//!     response::{IntoResponse, Html, Json, Headers},
//!     Router,
//! };
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
//! // A tuple of `HeaderMap` and something that implements `IntoResponse` can
//! // be used to override the headers
//! async fn with_headers() -> (HeaderMap, &'static str) {
//!     let mut headers = HeaderMap::new();
//!     headers.insert(
//!         HeaderName::from_static("x-foo"),
//!         HeaderValue::from_static("foo"),
//!     );
//!     (headers, "foo")
//! }
//!
//! // You can also override both status and headers at the same time
//! async fn with_headers_and_status() -> (StatusCode, HeaderMap, &'static str) {
//!     let mut headers = HeaderMap::new();
//!     headers.insert(
//!         HeaderName::from_static("x-foo"),
//!         HeaderValue::from_static("foo"),
//!     );
//!     (StatusCode::INTERNAL_SERVER_ERROR, headers, "foo")
//! }
//!
//! // `Headers` makes building the header map easier and `impl Trait` is easier
//! // so you don't have to write the whole type
//! async fn with_easy_headers() -> impl IntoResponse {
//!     Headers(vec![("x-foo", "foo")])
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
//! let app = Router::new()
//!     .route("/plain_text", get(plain_text))
//!     .route("/plain_text_string", get(plain_text_string))
//!     .route("/bytes", get(bytes))
//!     .route("/empty", get(empty))
//!     .route("/empty_with_status", get(empty_with_status))
//!     .route("/with_status", get(with_status))
//!     .route("/with_headers", get(with_headers))
//!     .route("/with_headers_and_status", get(with_headers_and_status))
//!     .route("/with_easy_headers", get(with_easy_headers))
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
//! ecosystem of middleware.
//!
//! If you're new to tower we recommend you read its [guides][tower-guides] for
//! a general introduction to tower and its concepts.
//!
//! ## To individual handlers
//!
//! A middleware can be applied to a single handler like so:
//!
//! ```rust,no_run
//! use axum::{
//!     handler::{get, Handler},
//!     Router,
//! };
//! use tower::limit::ConcurrencyLimitLayer;
//!
//! let app = Router::new()
//!     .route(
//!         "/",
//!         get(handler.layer(ConcurrencyLimitLayer::new(100))),
//!     );
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
//! use axum::{
//!     handler::{get, post},
//!     Router,
//! };
//! use tower::limit::ConcurrencyLimitLayer;
//!
//! async fn handler() {}
//!
//! let app = Router::new()
//!     .route("/", get(handler))
//!     .route("/foo", post(handler))
//!     .layer(ConcurrencyLimitLayer::new(100));
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! Note that [`Router::layer`] applies the middleware to all previously added
//! routes, of that particular `Router`. If you need multiple groups of routes
//! with different middleware build them separately and combine them with
//! [`Router::or`]:
//!
//! ```rust,no_run
//! use axum::{
//!     handler::{get, post},
//!     Router,
//! };
//! use tower::limit::ConcurrencyLimitLayer;
//! # type MyAuthLayer = tower::layer::util::Identity;
//!
//! async fn handler() {}
//!
//! let foo = Router::new()
//!     .route("/", get(handler))
//!     .route("/foo", post(handler))
//!     .layer(ConcurrencyLimitLayer::new(100));
//!
//! let bar = Router::new()
//!     .route("/requires-auth", get(handler))
//!     .layer(MyAuthLayer::new());
//!
//! let app = foo.or(bar);
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
//! use axum::{
//!     handler::{get, Handler},
//!     Router,
//! };
//! use tower::{
//!     BoxError, timeout::{TimeoutLayer, error::Elapsed},
//! };
//! use std::{borrow::Cow, time::Duration, convert::Infallible};
//! use http::StatusCode;
//!
//! let app = Router::new()
//!     .route(
//!         "/",
//!         get(handle
//!             .layer(TimeoutLayer::new(Duration::from_secs(30)))
//!             // `Timeout` uses `BoxError` as the error type
//!             .handle_error(|error: BoxError| {
//!                 // Check if the actual error type is `Elapsed` which
//!                 // `Timeout` returns
//!                 if error.is::<Elapsed>() {
//!                     return Ok::<_, Infallible>((
//!                         StatusCode::REQUEST_TIMEOUT,
//!                         "Request took too long".into(),
//!                     ));
//!                 }
//!
//!                 // If we encounter some error we don't handle return a generic
//!                 // error
//!                 return Ok::<_, Infallible>((
//!                     StatusCode::INTERNAL_SERVER_ERROR,
//!                     // `Cow` lets us return either `&str` or `String`
//!                     Cow::from(format!("Unhandled internal error: {}", error)),
//!                 ));
//!             })),
//!     );
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
//! See [`routing::Router::handle_error`] for more details.
//!
//! ## Applying multiple middleware
//!
//! [`tower::ServiceBuilder`] can be used to combine multiple middleware:
//!
//! ```rust,no_run
//! use axum::{
//!     body::Body,
//!     handler::get,
//!     http::Request,
//!     Router,
//! };
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
//! let app = Router::new()
//!     .route("/", get(|_: Request<Body>| async { /* ... */ }))
//!     .layer(middleware_stack);
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! ## Commonly used middleware
//!
//! [`tower::util`] and [`tower_http`] have a large collection of middleware that are compatible
//! with axum. Some commonly used are:
//!
//! ```rust,no_run
//! use axum::{
//!     body::{Body, BoxBody},
//!     handler::get,
//!     http::{Request, Response},
//!     Router,
//! };
//! use tower::{
//!     filter::AsyncFilterLayer,
//!     util::AndThenLayer,
//!     ServiceBuilder,
//! };
//! use std::convert::Infallible;
//! use tower_http::trace::TraceLayer;
//!
//! let middleware_stack = ServiceBuilder::new()
//!     // `TraceLayer` adds high level tracing and logging
//!     .layer(TraceLayer::new_for_http())
//!     // `AsyncFilterLayer` lets you asynchronously transform the request
//!     .layer(AsyncFilterLayer::new(map_request))
//!     // `AndThenLayer` lets you asynchronously transform the response
//!     .layer(AndThenLayer::new(map_response))
//!     .into_inner();
//!
//! async fn map_request(req: Request<Body>) -> Result<Request<Body>, Infallible> {
//!     Ok(req)
//! }
//!
//! async fn map_response(res: Response<BoxBody>) -> Result<Response<BoxBody>, Infallible> {
//!     Ok(res)
//! }
//!
//! let app = Router::new()
//!     .route("/", get(|| async { /* ... */ }))
//!     .layer(middleware_stack);
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! Additionally axum provides [`extract::extractor_middleware()`] for converting any extractor into
//! a middleware. Among other things, this can be useful for doing authorization. See
//! [`extract::extractor_middleware()`] for more details.
//!
//! ## Writing your own middleware
//!
//! You can also write you own middleware by implementing [`tower::Service`]:
//!
//! ```
//! use axum::{
//!     body::{Body, BoxBody},
//!     handler::get,
//!     http::{Request, Response},
//!     Router,
//! };
//! use futures::future::BoxFuture;
//! use tower::{Service, layer::layer_fn};
//! use std::task::{Context, Poll};
//!
//! #[derive(Clone)]
//! struct MyMiddleware<S> {
//!     inner: S,
//! }
//!
//! impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for MyMiddleware<S>
//! where
//!     S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
//!     S::Future: Send + 'static,
//!     ReqBody: Send + 'static,
//!     ResBody: Send + 'static,
//! {
//!     type Response = S::Response;
//!     type Error = S::Error;
//!     type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
//!
//!     fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
//!         self.inner.poll_ready(cx)
//!     }
//!
//!     fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
//!         println!("`MyMiddleware` called!");
//!
//!         // best practice is to clone the inner service like this
//!         // see https://github.com/tower-rs/tower/issues/547 for details
//!         let clone = self.inner.clone();
//!         let mut inner = std::mem::replace(&mut self.inner, clone);
//!
//!         Box::pin(async move {
//!             let res: Response<ResBody> = inner.call(req).await?;
//!
//!             println!("`MyMiddleware` received the response");
//!
//!             Ok(res)
//!         })
//!     }
//! }
//!
//! let app = Router::new()
//!     .route("/", get(|| async { /* ... */ }))
//!     .layer(layer_fn(|inner| MyMiddleware { inner }));
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
//! use axum::{
//!     AddExtensionLayer,
//!     extract,
//!     handler::get,
//!     Router,
//! };
//! use std::sync::Arc;
//!
//! struct State {
//!     // ...
//! }
//!
//! let shared_state = Arc::new(State { /* ... */ });
//!
//! let app = Router::new()
//!     .route("/", get(handler))
//!     .layer(AddExtensionLayer::new(shared_state));
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
//! - `headers`: Enables extracting typed headers via [`extract::TypedHeader`].
//! - `http2`: Enables hyper's `http2` feature.
//! - `multipart`: Enables parsing `multipart/form-data` requests with [`extract::Multipart`].
//! - `tower-log`: Enables `tower`'s `log` feature. Enabled by default.
//! - `ws`: Enables WebSockets support via [`extract::ws`].
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
//! [`Router::or`]: crate::routing::Router::or
//! [`axum::Server`]: hyper::server::Server
//! [`OriginalUri`]: crate::extract::OriginalUri
//! [`Service`]: tower::Service
//! [`Service::poll_ready`]: tower::Service::poll_ready
//! [`tower::Service`]: tower::Service
//! [`handle_error`]: routing::Router::handle_error
//! [tower-guides]: https://github.com/tower-rs/tower/tree/master/guides

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

#[doc(inline)]
pub use self::{error::Error, json::Json, routing::Router};

/// Alias for a type-erased error type.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
