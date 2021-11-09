//! Routing for [`Service`'s] based on HTTP methods.
//!
//! Most of the time applications will be written by composing
//! [handlers](crate::handler), however sometimes you might have some general
//! [`Service`] that you want to route requests to. That is enabled by the
//! functions in this module.
//!
//! # Example
//!
//! Using [`Redirect`] to redirect requests can be done like so:
//!
//! ```
//! use tower_http::services::Redirect;
//! use axum::{
//!     body::Body,
//!     routing::{get, service_method_routing as service},
//!     http::Request,
//!     Router,
//! };
//!
//! async fn handler(request: Request<Body>) { /* ... */ }
//!
//! let redirect_service = Redirect::<Body>::permanent("/new".parse().unwrap());
//!
//! let app = Router::new()
//!     .route("/old", service::get(redirect_service))
//!     .route("/new", get(handler));
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! # Regarding backpressure and `Service::poll_ready`
//!
//! Generally routing to one of multiple services and backpressure doesn't mix
//! well. Ideally you would want ensure a service is ready to receive a request
//! before calling it. However, in order to know which service to call, you need
//! the request...
//!
//! One approach is to not consider the router service itself ready until all
//! destination services are ready. That is the approach used by
//! [`tower::steer::Steer`].
//!
//! Another approach is to always consider all services ready (always return
//! `Poll::Ready(Ok(()))`) from `Service::poll_ready` and then actually drive
//! readiness inside the response future returned by `Service::call`. This works
//! well when your services don't care about backpressure and are always ready
//! anyway.
//!
//! axum expects that all services used in your app wont care about
//! backpressure and so it uses the latter strategy. However that means you
//! should avoid routing to a service (or using a middleware) that _does_ care
//! about backpressure. At the very least you should [load shed] so requests are
//! dropped quickly and don't keep piling up.
//!
//! It also means that if `poll_ready` returns an error then that error will be
//! returned in the response future from `call` and _not_ from `poll_ready`. In
//! that case, the underlying service will _not_ be discarded and will continue
//! to be used for future requests. Services that expect to be discarded if
//! `poll_ready` fails should _not_ be used with axum.
//!
//! One possible approach is to only apply backpressure sensitive middleware
//! around your entire app. This is possible because axum applications are
//! themselves services:
//!
//! ```rust
//! use axum::{
//!     routing::get,
//!     Router,
//! };
//! use tower::ServiceBuilder;
//! # let some_backpressure_sensitive_middleware =
//! #     tower::layer::util::Identity::new();
//!
//! async fn handler() { /* ... */ }
//!
//! let app = Router::new().route("/", get(handler));
//!
//! let app = ServiceBuilder::new()
//!     .layer(some_backpressure_sensitive_middleware)
//!     .service(app);
//! # async {
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! However when applying middleware around your whole application in this way
//! you have to take care that errors are still being handled with
//! appropriately.
//!
//! Also note that handlers created from async functions don't care about
//! backpressure and are always ready. So if you're not using any Tower
//! middleware you don't have to worry about any of this.
//!
//! [`Redirect`]: tower_http::services::Redirect
//! [load shed]: tower::load_shed
//! [`Service`'s]: tower::Service

use crate::{
    body::{box_body, BoxBody},
    routing::{MethodFilter, MethodNotAllowed},
    util::{Either, EitherProj},
    BoxError,
};
use bytes::Bytes;
use futures_util::ready;
use http::{Method, Request, Response};
use http_body::Empty;
use pin_project_lite::pin_project;
use std::{
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{util::Oneshot, ServiceExt as _};
use tower_service::Service;

/// Route requests with any standard HTTP method to the given service.
///
/// See [`get`] for an example.
///
/// Note that this only accepts the standard HTTP methods. If you need to
/// support non-standard methods you can route directly to a [`Service`].
pub fn any<S, B>(svc: S) -> MethodRouter<S, MethodNotAllowed<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::all(), svc)
}

/// Route `DELETE` requests to the given service.
///
/// See [`get`] for an example.
pub fn delete<S, B>(svc: S) -> MethodRouter<S, MethodNotAllowed<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::DELETE, svc)
}

/// Route `GET` requests to the given service.
///
/// # Example
///
/// ```rust
/// use axum::{
///     http::Request,
///     Router,
///     routing::service_method_routing as service,
/// };
/// use http::Response;
/// use std::convert::Infallible;
/// use hyper::Body;
///
/// let service = tower::service_fn(|request: Request<Body>| async {
///     Ok::<_, Infallible>(Response::new(Body::empty()))
/// });
///
/// // Requests to `GET /` will go to `service`.
/// let app = Router::new().route("/", service::get(service));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Note that `get` routes will also be called for `HEAD` requests but will have
/// the response body removed. Make sure to add explicit `HEAD` routes
/// afterwards.
pub fn get<S, B>(svc: S) -> MethodRouter<S, MethodNotAllowed<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::GET | MethodFilter::HEAD, svc)
}

/// Route `HEAD` requests to the given service.
///
/// See [`get`] for an example.
pub fn head<S, B>(svc: S) -> MethodRouter<S, MethodNotAllowed<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::HEAD, svc)
}

/// Route `OPTIONS` requests to the given service.
///
/// See [`get`] for an example.
pub fn options<S, B>(svc: S) -> MethodRouter<S, MethodNotAllowed<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::OPTIONS, svc)
}

/// Route `PATCH` requests to the given service.
///
/// See [`get`] for an example.
pub fn patch<S, B>(svc: S) -> MethodRouter<S, MethodNotAllowed<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::PATCH, svc)
}

/// Route `POST` requests to the given service.
///
/// See [`get`] for an example.
pub fn post<S, B>(svc: S) -> MethodRouter<S, MethodNotAllowed<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::POST, svc)
}

/// Route `PUT` requests to the given service.
///
/// See [`get`] for an example.
pub fn put<S, B>(svc: S) -> MethodRouter<S, MethodNotAllowed<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::PUT, svc)
}

/// Route `TRACE` requests to the given service.
///
/// See [`get`] for an example.
pub fn trace<S, B>(svc: S) -> MethodRouter<S, MethodNotAllowed<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::TRACE, svc)
}

/// Route requests with the given method to the service.
///
/// # Example
///
/// ```rust
/// use axum::{
///     http::Request,
///     routing::on,
///     Router,
///     routing::{MethodFilter, service_method_routing as service},
/// };
/// use http::Response;
/// use std::convert::Infallible;
/// use hyper::Body;
///
/// let service = tower::service_fn(|request: Request<Body>| async {
///     Ok::<_, Infallible>(Response::new(Body::empty()))
/// });
///
/// // Requests to `POST /` will go to `service`.
/// let app = Router::new().route("/", service::on(MethodFilter::POST, service));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn on<S, B>(method: MethodFilter, svc: S) -> MethodRouter<S, MethodNotAllowed<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    MethodRouter {
        method,
        svc,
        fallback: MethodNotAllowed::new(),
        _request_body: PhantomData,
    }
}

/// A [`Service`] that accepts requests based on a [`MethodFilter`] and allows
/// chaining additional services.
pub struct MethodRouter<S, F, B> {
    pub(crate) method: MethodFilter,
    pub(crate) svc: S,
    pub(crate) fallback: F,
    pub(crate) _request_body: PhantomData<fn() -> B>,
}

impl<S, F, B> fmt::Debug for MethodRouter<S, F, B>
where
    S: fmt::Debug,
    F: fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MethodRouter")
            .field("method", &self.method)
            .field("svc", &self.svc)
            .field("fallback", &self.fallback)
            .finish()
    }
}

impl<S, F, B> Clone for MethodRouter<S, F, B>
where
    S: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            method: self.method,
            svc: self.svc.clone(),
            fallback: self.fallback.clone(),
            _request_body: PhantomData,
        }
    }
}

impl<S, F, B> MethodRouter<S, F, B> {
    /// Chain an additional service that will accept all requests regardless of
    /// its HTTP method.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn any<T>(self, svc: T) -> MethodRouter<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::all(), svc)
    }

    /// Chain an additional service that will only accept `DELETE` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn delete<T>(self, svc: T) -> MethodRouter<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::DELETE, svc)
    }

    /// Chain an additional service that will only accept `GET` requests.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{
    ///     http::Request,
    ///     Router,
    ///     routing::{MethodFilter, on, service_method_routing as service},
    /// };
    /// use http::Response;
    /// use std::convert::Infallible;
    /// use hyper::Body;
    ///
    /// let service = tower::service_fn(|request: Request<Body>| async {
    ///     Ok::<_, Infallible>(Response::new(Body::empty()))
    /// });
    ///
    /// let other_service = tower::service_fn(|request: Request<Body>| async {
    ///     Ok::<_, Infallible>(Response::new(Body::empty()))
    /// });
    ///
    /// // Requests to `GET /` will go to `service` and `POST /` will go to
    /// // `other_service`.
    /// let app = Router::new().route("/", service::post(service).get(other_service));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// Note that `get` routes will also be called for `HEAD` requests but will have
    /// the response body removed. Make sure to add explicit `HEAD` routes
    /// afterwards.
    pub fn get<T>(self, svc: T) -> MethodRouter<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::GET | MethodFilter::HEAD, svc)
    }

    /// Chain an additional service that will only accept `HEAD` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn head<T>(self, svc: T) -> MethodRouter<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::HEAD, svc)
    }

    /// Chain an additional service that will only accept `OPTIONS` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn options<T>(self, svc: T) -> MethodRouter<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::OPTIONS, svc)
    }

    /// Chain an additional service that will only accept `PATCH` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn patch<T>(self, svc: T) -> MethodRouter<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::PATCH, svc)
    }

    /// Chain an additional service that will only accept `POST` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn post<T>(self, svc: T) -> MethodRouter<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::POST, svc)
    }

    /// Chain an additional service that will only accept `PUT` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn put<T>(self, svc: T) -> MethodRouter<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::PUT, svc)
    }

    /// Chain an additional service that will only accept `TRACE` requests.
    ///
    /// See [`MethodRouter::get`] for an example.
    pub fn trace<T>(self, svc: T) -> MethodRouter<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::TRACE, svc)
    }

    /// Chain an additional service that will accept requests matching the given
    /// `MethodFilter`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{
    ///     http::Request,
    ///     Router,
    ///     routing::{MethodFilter, on, service_method_routing as service},
    /// };
    /// use http::Response;
    /// use std::convert::Infallible;
    /// use hyper::Body;
    ///
    /// let service = tower::service_fn(|request: Request<Body>| async {
    ///     Ok::<_, Infallible>(Response::new(Body::empty()))
    /// });
    ///
    /// let other_service = tower::service_fn(|request: Request<Body>| async {
    ///     Ok::<_, Infallible>(Response::new(Body::empty()))
    /// });
    ///
    /// // Requests to `DELETE /` will go to `service`
    /// let app = Router::new().route("/", service::on(MethodFilter::DELETE, service));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn on<T>(self, method: MethodFilter, svc: T) -> MethodRouter<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        MethodRouter {
            method,
            svc,
            fallback: self,
            _request_body: PhantomData,
        }
    }
}

impl<S, F, B, ResBody> Service<Request<B>> for MethodRouter<S, F, B>
where
    S: Service<Request<B>, Response = Response<ResBody>> + Clone,
    ResBody: http_body::Body<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error> + Clone,
{
    type Response = Response<BoxBody>;
    type Error = S::Error;
    type Future = MethodRouterFuture<S, F, B>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let req_method = req.method().clone();

        let f = if self.method.matches(req.method()) {
            let fut = self.svc.clone().oneshot(req);
            Either::A { inner: fut }
        } else {
            let fut = self.fallback.clone().oneshot(req);
            Either::B { inner: fut }
        };

        MethodRouterFuture {
            inner: f,
            req_method,
        }
    }
}

pin_project! {
    /// The response future for [`MethodRouter`].
    pub struct MethodRouterFuture<S, F, B>
    where
        S: Service<Request<B>>,
        F: Service<Request<B>>
    {
        #[pin]
        pub(super) inner: Either<
            Oneshot<S, Request<B>>,
            Oneshot<F, Request<B>>,
        >,
        pub(super) req_method: Method,
    }
}

impl<S, F, B, ResBody> Future for MethodRouterFuture<S, F, B>
where
    S: Service<Request<B>, Response = Response<ResBody>> + Clone,
    ResBody: http_body::Body<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error>,
{
    type Output = Result<Response<BoxBody>, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let response = match this.inner.project() {
            EitherProj::A { inner } => ready!(inner.poll(cx))?.map(box_body),
            EitherProj::B { inner } => ready!(inner.poll(cx))?,
        };

        if this.req_method == &Method::HEAD {
            let response = response.map(|_| box_body(Empty::new()));
            Poll::Ready(Ok(response))
        } else {
            Poll::Ready(Ok(response))
        }
    }
}

#[test]
fn traits() {
    use crate::test_helpers::*;

    assert_send::<MethodRouter<(), (), NotSendSync>>();
    assert_sync::<MethodRouter<(), (), NotSendSync>>();
}
