//! Routing between [`Service`]s.

use crate::{
    body::{box_body, BoxBody},
    buffer::MpscBuffer,
    response::IntoResponse,
    util::ByteStr,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::future;
use http::{Method, Request, Response, StatusCode, Uri};
use pin_project::pin_project;
use regex::Regex;
use std::{
    borrow::Cow,
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{
    util::{BoxService, Oneshot, ServiceExt},
    BoxError, Layer, Service, ServiceBuilder,
};
use tower_http::map_response_body::MapResponseBodyLayer;

/// A filter that matches one or more HTTP methods.
#[derive(Debug, Copy, Clone)]
pub enum MethodFilter {
    /// Match any method.
    Any,
    /// Match `CONNECT` requests.
    Connect,
    /// Match `DELETE` requests.
    Delete,
    /// Match `GET` requests.
    Get,
    /// Match `HEAD` requests.
    Head,
    /// Match `OPTIONS` requests.
    Options,
    /// Match `PATCH` requests.
    Patch,
    /// Match `POST` requests.
    Post,
    /// Match `PUT` requests.
    Put,
    /// Match `TRACE` requests.
    Trace,
}

impl MethodFilter {
    #[allow(clippy::match_like_matches_macro)]
    pub(crate) fn matches(self, method: &Method) -> bool {
        match (self, method) {
            (MethodFilter::Any, _)
            | (MethodFilter::Connect, &Method::CONNECT)
            | (MethodFilter::Delete, &Method::DELETE)
            | (MethodFilter::Get, &Method::GET)
            | (MethodFilter::Head, &Method::HEAD)
            | (MethodFilter::Options, &Method::OPTIONS)
            | (MethodFilter::Patch, &Method::PATCH)
            | (MethodFilter::Post, &Method::POST)
            | (MethodFilter::Put, &Method::PUT)
            | (MethodFilter::Trace, &Method::TRACE) => true,
            _ => false,
        }
    }
}

/// A route that sends requests to one of two [`Service`]s depending on the
/// path.
///
/// Created with [`route`](crate::route). See that function for more details.
#[derive(Debug, Clone)]
pub struct Route<S, F> {
    pub(crate) pattern: PathPattern,
    pub(crate) svc: S,
    pub(crate) fallback: F,
}

/// Trait for building routers.
// TODO(david): this name isn't great
#[async_trait]
pub trait RoutingDsl: crate::sealed::Sealed + Sized {
    /// Add another route to the router.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::prelude::*;
    ///
    /// async fn first_handler() { /* ... */ }
    ///
    /// async fn second_handler() { /* ... */ }
    ///
    /// async fn third_handler() { /* ... */ }
    ///
    /// // `GET /` goes to `first_handler`, `POST /` goes to `second_handler`,
    /// // and `GET /foo` goes to third_handler.
    /// let app = route("/", get(first_handler).post(second_handler))
    ///     .route("/foo", get(third_handler));
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    fn route<T, B>(self, description: &str, svc: T) -> Route<T, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        Route {
            pattern: PathPattern::new(description),
            svc,
            fallback: self,
        }
    }

    /// Nest another service inside this router at the given path.
    ///
    /// See [`nest`] for more details.
    fn nest<T, B>(self, description: &str, svc: T) -> Nested<T, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        Nested {
            pattern: PathPattern::new(description),
            svc,
            fallback: self,
        }
    }

    /// Create a boxed route trait object.
    ///
    /// This makes it easier to name the types of routers to, for example,
    /// return them from functions:
    ///
    /// ```rust
    /// use axum::{routing::BoxRoute, body::Body, prelude::*};
    ///
    /// async fn first_handler() { /* ... */ }
    ///
    /// async fn second_handler() { /* ... */ }
    ///
    /// async fn third_handler() { /* ... */ }
    ///
    /// fn app() -> BoxRoute<Body> {
    ///     route("/", get(first_handler).post(second_handler))
    ///         .route("/foo", get(third_handler))
    ///         .boxed()
    /// }
    /// ```
    ///
    /// It also helps with compile times when you have a very large number of
    /// routes.
    fn boxed<ReqBody, ResBody>(self) -> BoxRoute<ReqBody, Self::Error>
    where
        Self: Service<Request<ReqBody>, Response = Response<ResBody>> + Send + 'static,
        <Self as Service<Request<ReqBody>>>::Error: Into<BoxError> + Send + Sync,
        <Self as Service<Request<ReqBody>>>::Future: Send,
        ReqBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        ReqBody::Error: Into<BoxError> + Send + Sync + 'static,
        ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        ResBody::Error: Into<BoxError> + Send + Sync + 'static,
    {
        ServiceBuilder::new()
            .layer_fn(BoxRoute)
            .layer_fn(MpscBuffer::new)
            .layer(BoxService::layer())
            .layer(MapResponseBodyLayer::new(box_body))
            .service(self)
    }

    /// Apply a [`tower::Layer`] to the router.
    ///
    /// All requests to the router will be processed by the layer's
    /// corresponding middleware.
    ///
    /// This can be used to add additional processing to a request for a group
    /// of routes.
    ///
    /// Note this differes from [`handler::Layered`](crate::handler::Layered)
    /// which adds a middleware to a single handler.
    ///
    /// # Example
    ///
    /// Adding the [`tower::limit::ConcurrencyLimit`] middleware to a group of
    /// routes can be done like so:
    ///
    /// ```rust
    /// use axum::prelude::*;
    /// use tower::limit::{ConcurrencyLimitLayer, ConcurrencyLimit};
    ///
    /// async fn first_handler() { /* ... */ }
    ///
    /// async fn second_handler() { /* ... */ }
    ///
    /// async fn third_handler() { /* ... */ }
    ///
    /// // All requests to `handler` and `other_handler` will be sent through
    /// // `ConcurrencyLimit`
    /// let app = route("/", get(first_handler))
    ///     .route("/foo", get(second_handler))
    ///     .layer(ConcurrencyLimitLayer::new(64))
    ///     // Request to `GET /bar` will go directly to `third_handler` and
    ///     // wont be sent through `ConcurrencyLimit`
    ///     .route("/bar", get(third_handler));
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// This is commonly used to add middleware such as tracing/logging to your
    /// entire app:
    ///
    /// ```rust
    /// use axum::prelude::*;
    /// use tower_http::trace::TraceLayer;
    ///
    /// async fn first_handler() { /* ... */ }
    ///
    /// async fn second_handler() { /* ... */ }
    ///
    /// async fn third_handler() { /* ... */ }
    ///
    /// let app = route("/", get(first_handler))
    ///     .route("/foo", get(second_handler))
    ///     .route("/bar", get(third_handler))
    ///     .layer(TraceLayer::new_for_http());
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    fn layer<L>(self, layer: L) -> Layered<L::Service>
    where
        L: Layer<Self>,
    {
        Layered::new(layer.layer(self))
    }

    /// Convert this router into a [`MakeService`], that is a [`Service`] who's
    /// response is another service.
    ///
    /// This is useful when running your application with hyper's
    /// [`Server`](hyper::server::Server):
    ///
    /// ```
    /// use axum::prelude::*;
    ///
    /// let app = route("/", get(|| async { "Hi!" }));
    ///
    /// # async {
    /// hyper::Server::bind(&"0.0.0.0:3000".parse().unwrap())
    ///     .serve(app.into_make_service())
    ///     .await
    ///     .expect("server failed");
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    fn into_make_service(self) -> tower::make::Shared<Self>
    where
        Self: Clone,
    {
        tower::make::Shared::new(self)
    }
}

impl<S, F> RoutingDsl for Route<S, F> {}

impl<S, F> crate::sealed::Sealed for Route<S, F> {}

impl<S, F, B> Service<Request<B>> for Route<S, F>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error> + Clone,
{
    type Response = Response<BoxBody>;
    type Error = S::Error;
    type Future = RouteFuture<S, F, B>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        if let Some(captures) = self.pattern.full_match(req.uri().path()) {
            insert_url_params(&mut req, captures);
            let fut = self.svc.clone().oneshot(req);
            RouteFuture::a(fut)
        } else {
            let fut = self.fallback.clone().oneshot(req);
            RouteFuture::b(fut)
        }
    }
}

/// The response future for [`Route`].
#[pin_project]
#[derive(Debug)]
pub struct RouteFuture<S, F, B>(#[pin] RouteFutureInner<S, F, B>)
where
    S: Service<Request<B>>,
    F: Service<Request<B>>;

impl<S, F, B> RouteFuture<S, F, B>
where
    S: Service<Request<B>>,
    F: Service<Request<B>>,
{
    pub(crate) fn a(a: Oneshot<S, Request<B>>) -> Self {
        RouteFuture(RouteFutureInner::A(a))
    }

    pub(crate) fn b(b: Oneshot<F, Request<B>>) -> Self {
        RouteFuture(RouteFutureInner::B(b))
    }
}

#[pin_project(project = RouteFutureInnerProj)]
#[derive(Debug)]
enum RouteFutureInner<S, F, B>
where
    S: Service<Request<B>>,
    F: Service<Request<B>>,
{
    A(#[pin] Oneshot<S, Request<B>>),
    B(#[pin] Oneshot<F, Request<B>>),
}

impl<S, F, B> Future for RouteFuture<S, F, B>
where
    S: Service<Request<B>, Response = Response<BoxBody>>,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error>,
{
    type Output = Result<Response<BoxBody>, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().0.project() {
            RouteFutureInnerProj::A(inner) => inner.poll(cx),
            RouteFutureInnerProj::B(inner) => inner.poll(cx),
        }
    }
}

#[derive(Debug)]
pub(crate) struct UrlParams(pub(crate) Vec<(ByteStr, ByteStr)>);

fn insert_url_params<B>(req: &mut Request<B>, params: Vec<(String, String)>) {
    let params = params
        .into_iter()
        .map(|(k, v)| (ByteStr::new(k), ByteStr::new(v)));

    if let Some(current) = req.extensions_mut().get_mut::<Option<UrlParams>>() {
        let mut current = current.take().unwrap();
        current.0.extend(params);
        req.extensions_mut().insert(Some(current));
    } else {
        req.extensions_mut()
            .insert(Some(UrlParams(params.collect())));
    }
}

/// A [`Service`] that responds with `404 Not Found` to all requests.
///
/// This is used as the bottom service in a router stack. You shouldn't have to
/// use to manually.
pub struct EmptyRouter<E = Infallible>(PhantomData<fn() -> E>);

impl<E> EmptyRouter<E> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }
}

impl<E> Clone for EmptyRouter<E> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<E> Copy for EmptyRouter<E> {}

impl<E> fmt::Debug for EmptyRouter<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EmptyRouter").finish()
    }
}

impl<E> RoutingDsl for EmptyRouter<E> {}

impl<E> crate::sealed::Sealed for EmptyRouter<E> {}

impl<B, E> Service<Request<B>> for EmptyRouter<E> {
    type Response = Response<BoxBody>;
    type Error = E;
    type Future = EmptyRouterFuture<E>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Request<B>) -> Self::Future {
        let mut res = Response::new(crate::body::empty());
        *res.status_mut() = StatusCode::NOT_FOUND;
        EmptyRouterFuture(future::ok(res))
    }
}

opaque_future! {
    /// Response future for [`EmptyRouter`].
    pub type EmptyRouterFuture<E> =
        future::Ready<Result<Response<BoxBody>, E>>;
}

#[derive(Debug, Clone)]
pub(crate) struct PathPattern(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    full_path_regex: Regex,
    capture_group_names: Box<[Bytes]>,
}

impl PathPattern {
    pub(crate) fn new(pattern: &str) -> Self {
        assert!(
            pattern.starts_with('/'),
            "Route description must start with a `/`"
        );

        let mut capture_group_names = Vec::new();

        let pattern = pattern
            .split('/')
            .map(|part| {
                if let Some(key) = part.strip_prefix(':') {
                    capture_group_names.push(Bytes::copy_from_slice(key.as_bytes()));

                    Cow::Owned(format!("(?P<{}>[^/]*)", key))
                } else {
                    Cow::Borrowed(part)
                }
            })
            .collect::<Vec<_>>()
            .join("/");

        let full_path_regex =
            Regex::new(&format!("^{}", pattern)).expect("invalid regex generated from route");

        Self(Arc::new(Inner {
            full_path_regex,
            capture_group_names: capture_group_names.into(),
        }))
    }

    pub(crate) fn full_match(&self, path: &str) -> Option<Captures> {
        self.do_match(path).and_then(|match_| {
            if match_.full_match {
                Some(match_.captures)
            } else {
                None
            }
        })
    }

    pub(crate) fn prefix_match<'a>(&self, path: &'a str) -> Option<(&'a str, Captures)> {
        self.do_match(path)
            .map(|match_| (match_.matched, match_.captures))
    }

    fn do_match<'a>(&self, path: &'a str) -> Option<Match<'a>> {
        self.0.full_path_regex.captures(path).map(|captures| {
            let matched = captures.get(0).unwrap();
            let full_match = matched.as_str() == path;

            let captures = self
                .0
                .capture_group_names
                .iter()
                .map(|bytes| {
                    std::str::from_utf8(bytes)
                        .expect("bytes were created from str so is valid utf-8")
                })
                .filter_map(|name| captures.name(name).map(|value| (name, value.as_str())))
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect::<Vec<_>>();

            Match {
                captures,
                full_match,
                matched: matched.as_str(),
            }
        })
    }
}

struct Match<'a> {
    captures: Captures,
    // true if regex matched whole path, false if it only matched a prefix
    full_match: bool,
    matched: &'a str,
}

type Captures = Vec<(String, String)>;

/// A boxed route trait object.
///
/// See [`RoutingDsl::boxed`] for more details.
pub struct BoxRoute<B, E = Infallible>(
    MpscBuffer<BoxService<Request<B>, Response<BoxBody>, E>, Request<B>>,
);

impl<B, E> Clone for BoxRoute<B, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<B, E> fmt::Debug for BoxRoute<B, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxRoute").finish()
    }
}

impl<B, E> RoutingDsl for BoxRoute<B, E> {}

impl<B, E> crate::sealed::Sealed for BoxRoute<B, E> {}

impl<B, E> Service<Request<B>> for BoxRoute<B, E>
where
    E: Into<BoxError>,
{
    type Response = Response<BoxBody>;
    type Error = E;
    type Future = BoxRouteFuture<B, E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        BoxRouteFuture {
            inner: self.0.clone().oneshot(req),
        }
    }
}

/// The response future for [`BoxRoute`].
#[pin_project]
pub struct BoxRouteFuture<B, E>
where
    E: Into<BoxError>,
{
    #[pin]
    inner:
        Oneshot<MpscBuffer<BoxService<Request<B>, Response<BoxBody>, E>, Request<B>>, Request<B>>,
}

impl<B, E> Future for BoxRouteFuture<B, E>
where
    E: Into<BoxError>,
{
    type Output = Result<Response<BoxBody>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

impl<B, E> fmt::Debug for BoxRouteFuture<B, E>
where
    E: Into<BoxError>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxRouteFuture").finish()
    }
}

/// A [`Service`] created from a router by applying a Tower middleware.
///
/// Created with [`RoutingDsl::layer`]. See that method for more details.
pub struct Layered<S> {
    inner: S,
}

impl<S> Layered<S> {
    fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Clone for Layered<S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.inner.clone())
    }
}

impl<S> fmt::Debug for Layered<S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Layered")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<S> RoutingDsl for Layered<S> {}

impl<S> crate::sealed::Sealed for Layered<S> {}

impl<S> Layered<S> {
    /// Create a new [`Layered`] service where errors will be handled using the
    /// given closure.
    ///
    /// This is used to convert errors to responses rather than simply
    /// terminating the connection.
    ///
    /// That can be done using `handle_error` like so:
    ///
    /// ```rust
    /// use axum::prelude::*;
    /// use http::StatusCode;
    /// use tower::{BoxError, timeout::TimeoutLayer};
    /// use std::{convert::Infallible, time::Duration};
    ///
    /// async fn handler() { /* ... */ }
    ///
    /// // `Timeout` will fail with `BoxError` if the timeout elapses...
    /// let layered_app = route("/", get(handler))
    ///     .layer(TimeoutLayer::new(Duration::from_secs(30)));
    ///
    /// // ...so we should handle that error
    /// let with_errors_handled = layered_app.handle_error(|error: BoxError| {
    ///     if error.is::<tower::timeout::error::Elapsed>() {
    ///         Ok::<_, Infallible>((
    ///             StatusCode::REQUEST_TIMEOUT,
    ///             "request took too long".to_string(),
    ///         ))
    ///     } else {
    ///         Ok::<_, Infallible>((
    ///             StatusCode::INTERNAL_SERVER_ERROR,
    ///             format!("Unhandled internal error: {}", error),
    ///         ))
    ///     }
    /// });
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap())
    /// #     .serve(with_errors_handled.into_make_service())
    /// #     .await
    /// #     .unwrap();
    /// # };
    /// ```
    ///
    /// The closure must return `Result<T, E>` where `T` implements [`IntoResponse`].
    ///
    /// You can also return `Err(_)` if you don't wish to handle the error:
    ///
    /// ```rust
    /// use axum::prelude::*;
    /// use http::StatusCode;
    /// use tower::{BoxError, timeout::TimeoutLayer};
    /// use std::time::Duration;
    ///
    /// async fn handler() { /* ... */ }
    ///
    /// let layered_app = route("/", get(handler))
    ///     .layer(TimeoutLayer::new(Duration::from_secs(30)));
    ///
    /// let with_errors_handled = layered_app.handle_error(|error: BoxError| {
    ///     if error.is::<tower::timeout::error::Elapsed>() {
    ///         Ok((
    ///             StatusCode::REQUEST_TIMEOUT,
    ///             "request took too long".to_string(),
    ///         ))
    ///     } else {
    ///         // keep the error as is
    ///         Err(error)
    ///     }
    /// });
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap())
    /// #     .serve(with_errors_handled.into_make_service())
    /// #     .await
    /// #     .unwrap();
    /// # };
    /// ```
    pub fn handle_error<F, ReqBody, ResBody, Res, E>(
        self,
        f: F,
    ) -> crate::service::HandleError<S, F, ReqBody>
    where
        S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone,
        F: FnOnce(S::Error) -> Result<Res, E>,
        Res: IntoResponse,
        ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        ResBody::Error: Into<BoxError> + Send + Sync + 'static,
    {
        crate::service::HandleError::new(self.inner, f)
    }
}

impl<S, R> Service<R> for Layered<S>
where
    S: Service<R>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: R) -> Self::Future {
        self.inner.call(req)
    }
}

/// Nest a group of routes (or a [`Service`]) at some path.
///
/// This allows you to break your application into smaller pieces and compose
/// them together. This will strip the matching prefix from the URL so the
/// nested route will only see the part of URL:
///
/// ```
/// use axum::{routing::nest, prelude::*};
/// use http::Uri;
///
/// async fn users_get(uri: Uri) {
///     // `users_get` doesn't see the whole URL. `nest` will strip the matching
///     // `/api` prefix.
///     assert_eq!(uri.path(), "/users");
/// }
///
/// async fn users_post() {}
///
/// async fn careers() {}
///
/// let users_api = route("/users", get(users_get).post(users_post));
///
/// let app = nest("/api", users_api).route("/careers", get(careers));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Take care when using `nest` together with dynamic routes as nesting also
/// captures from the outer routes:
///
/// ```
/// use axum::{routing::nest, prelude::*};
///
/// async fn users_get(params: extract::UrlParamsMap) {
///     // Both `version` and `id` were captured even though `users_api` only
///     // explicitly captures `id`.
///     let version = params.get("version");
///     let id = params.get("id");
/// }
///
/// let users_api = route("/users/:id", get(users_get));
///
/// let app = nest("/:version/api", users_api);
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// `nest` also accepts any [`Service`]. This can for example be used with
/// [`tower_http::services::ServeDir`] to serve static files from a directory:
///
/// ```
/// use axum::{
///     routing::nest, service::{get, ServiceExt}, prelude::*,
/// };
/// use tower_http::services::ServeDir;
///
/// // Serves files inside the `public` directory at `GET /public/*`
/// let serve_dir_service = ServeDir::new("public");
///
/// let app = nest("/public", get(serve_dir_service));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If necessary you can use [`RoutingDsl::boxed`] to box a group of routes
/// making the type easier to name. This is sometimes useful when working with
/// `nest`.
pub fn nest<S, B>(description: &str, svc: S) -> Nested<S, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    Nested {
        pattern: PathPattern::new(description),
        svc,
        fallback: EmptyRouter::new(),
    }
}

/// A [`Service`] that has been nested inside a router at some path.
///
/// Created with [`nest`] or [`RoutingDsl::nest`].
#[derive(Debug, Clone)]
pub struct Nested<S, F> {
    pattern: PathPattern,
    svc: S,
    fallback: F,
}

impl<S, F> RoutingDsl for Nested<S, F> {}

impl<S, F> crate::sealed::Sealed for Nested<S, F> {}

impl<S, F, B> Service<Request<B>> for Nested<S, F>
where
    S: Service<Request<B>, Response = Response<BoxBody>> + Clone,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error> + Clone,
{
    type Response = Response<BoxBody>;
    type Error = S::Error;
    type Future = RouteFuture<S, F, B>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        if let Some((prefix, captures)) = self.pattern.prefix_match(req.uri().path()) {
            let without_prefix = strip_prefix(req.uri(), prefix);
            *req.uri_mut() = without_prefix;

            insert_url_params(&mut req, captures);
            let fut = self.svc.clone().oneshot(req);
            RouteFuture::a(fut)
        } else {
            let fut = self.fallback.clone().oneshot(req);
            RouteFuture::b(fut)
        }
    }
}

fn strip_prefix(uri: &Uri, prefix: &str) -> Uri {
    let path_and_query = if let Some(path_and_query) = uri.path_and_query() {
        let mut new_path = if let Some(path) = path_and_query.path().strip_prefix(prefix) {
            path
        } else {
            path_and_query.path()
        };

        if new_path.is_empty() {
            new_path = "/";
        }

        if let Some(query) = path_and_query.query() {
            Some(
                format!("{}?{}", new_path, query)
                    .parse::<http::uri::PathAndQuery>()
                    .unwrap(),
            )
        } else {
            Some(new_path.parse().unwrap())
        }
    } else {
        None
    };

    let mut parts = http::uri::Parts::default();
    parts.scheme = uri.scheme().cloned();
    parts.authority = uri.authority().cloned();
    parts.path_and_query = path_and_query;

    Uri::from_parts(parts).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing() {
        assert_match("/", "/");

        assert_match("/foo", "/foo");
        assert_match("/foo/", "/foo/");
        refute_match("/foo", "/foo/");
        refute_match("/foo/", "/foo");

        assert_match("/foo/bar", "/foo/bar");
        refute_match("/foo/bar/", "/foo/bar");
        refute_match("/foo/bar", "/foo/bar/");

        assert_match("/:value", "/foo");
        assert_match("/users/:id", "/users/1");
        assert_match("/users/:id/action", "/users/42/action");
        refute_match("/users/:id/action", "/users/42");
        refute_match("/users/:id", "/users/42/action");
    }

    fn assert_match(route_spec: &'static str, path: &'static str) {
        let route = PathPattern::new(route_spec);
        assert!(
            route.full_match(path).is_some(),
            "`{}` doesn't match `{}`",
            path,
            route_spec
        );
    }

    fn refute_match(route_spec: &'static str, path: &'static str) {
        let route = PathPattern::new(route_spec);
        assert!(
            route.full_match(path).is_none(),
            "`{}` did match `{}` (but shouldn't)",
            path,
            route_spec
        );
    }
}
