//! Use Tower [`Service`]s to handle requests.
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
//!     handler::get,
//!     http::Request,
//!     route,
//!     service,
//! };
//!
//! async fn handler(request: Request<Body>) { /* ... */ }
//!
//! let redirect_service = Redirect::<Body>::permanent("/new".parse().unwrap());
//!
//! let app = route("/old", service::get(redirect_service))
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
//!     handler::get,
//!     route,
//! };
//! use tower::ServiceBuilder;
//! # let some_backpressure_sensitive_middleware =
//! #     tower::layer::util::Identity::new();
//!
//! async fn handler() { /* ... */ }
//!
//! let app = route("/", get(handler));
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

use crate::{
    body::BoxBody,
    response::IntoResponse,
    routing::{EmptyRouter, MethodFilter},
};
use bytes::Bytes;
use http::{Request, Response};
use std::{
    fmt,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower::{util::Oneshot, BoxError, Service, ServiceExt as _};

pub mod future;

/// Route requests to the given service regardless of the HTTP method.
///
/// See [`get`] for an example.
pub fn any<S, B>(svc: S) -> OnMethod<S, EmptyRouter<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::all(), svc)
}

/// Route `CONNECT` requests to the given service.
///
/// See [`get`] for an example.
pub fn connect<S, B>(svc: S) -> OnMethod<S, EmptyRouter<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::CONNECT, svc)
}

/// Route `DELETE` requests to the given service.
///
/// See [`get`] for an example.
pub fn delete<S, B>(svc: S) -> OnMethod<S, EmptyRouter<S::Error>, B>
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
///     route,
///     service,
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
/// let app = route("/", service::get(service));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Note that `get` routes will also be called for `HEAD` requests but will have
/// the response body removed. Make sure to add explicit `HEAD` routes
/// afterwards.
pub fn get<S, B>(svc: S) -> OnMethod<S, EmptyRouter<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::GET | MethodFilter::HEAD, svc)
}

/// Route `HEAD` requests to the given service.
///
/// See [`get`] for an example.
pub fn head<S, B>(svc: S) -> OnMethod<S, EmptyRouter<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::HEAD, svc)
}

/// Route `OPTIONS` requests to the given service.
///
/// See [`get`] for an example.
pub fn options<S, B>(svc: S) -> OnMethod<S, EmptyRouter<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::OPTIONS, svc)
}

/// Route `PATCH` requests to the given service.
///
/// See [`get`] for an example.
pub fn patch<S, B>(svc: S) -> OnMethod<S, EmptyRouter<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::PATCH, svc)
}

/// Route `POST` requests to the given service.
///
/// See [`get`] for an example.
pub fn post<S, B>(svc: S) -> OnMethod<S, EmptyRouter<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::POST, svc)
}

/// Route `PUT` requests to the given service.
///
/// See [`get`] for an example.
pub fn put<S, B>(svc: S) -> OnMethod<S, EmptyRouter<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::PUT, svc)
}

/// Route `TRACE` requests to the given service.
///
/// See [`get`] for an example.
pub fn trace<S, B>(svc: S) -> OnMethod<S, EmptyRouter<S::Error>, B>
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
///     handler::on,
///     service,
///     route,
///     routing::MethodFilter,
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
/// let app = route("/", service::on(MethodFilter::POST, service));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn on<S, B>(method: MethodFilter, svc: S) -> OnMethod<S, EmptyRouter<S::Error>, B>
where
    S: Service<Request<B>> + Clone,
{
    OnMethod {
        method,
        svc,
        fallback: EmptyRouter::method_not_allowed(),
        _request_body: PhantomData,
    }
}

/// A [`Service`] that accepts requests based on a [`MethodFilter`] and allows
/// chaining additional services.
#[derive(Debug)] // TODO(david): don't require debug for B
pub struct OnMethod<S, F, B> {
    pub(crate) method: MethodFilter,
    pub(crate) svc: S,
    pub(crate) fallback: F,
    pub(crate) _request_body: PhantomData<fn() -> B>,
}

impl<S, F, B> Clone for OnMethod<S, F, B>
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

impl<S, F, B> OnMethod<S, F, B> {
    /// Chain an additional service that will accept all requests regardless of
    /// its HTTP method.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn any<T>(self, svc: T) -> OnMethod<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::all(), svc)
    }

    /// Chain an additional service that will only accept `CONNECT` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn connect<T>(self, svc: T) -> OnMethod<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::CONNECT, svc)
    }

    /// Chain an additional service that will only accept `DELETE` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn delete<T>(self, svc: T) -> OnMethod<T, Self, B>
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
    ///     handler::on,
    ///     service,
    ///     route,
    ///     routing::MethodFilter,
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
    /// let app = route("/", service::post(service).get(other_service));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// Note that `get` routes will also be called for `HEAD` requests but will have
    /// the response body removed. Make sure to add explicit `HEAD` routes
    /// afterwards.
    pub fn get<T>(self, svc: T) -> OnMethod<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::GET | MethodFilter::HEAD, svc)
    }

    /// Chain an additional service that will only accept `HEAD` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn head<T>(self, svc: T) -> OnMethod<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::HEAD, svc)
    }

    /// Chain an additional service that will only accept `OPTIONS` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn options<T>(self, svc: T) -> OnMethod<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::OPTIONS, svc)
    }

    /// Chain an additional service that will only accept `PATCH` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn patch<T>(self, svc: T) -> OnMethod<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::PATCH, svc)
    }

    /// Chain an additional service that will only accept `POST` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn post<T>(self, svc: T) -> OnMethod<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::POST, svc)
    }

    /// Chain an additional service that will only accept `PUT` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn put<T>(self, svc: T) -> OnMethod<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::PUT, svc)
    }

    /// Chain an additional service that will only accept `TRACE` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn trace<T>(self, svc: T) -> OnMethod<T, Self, B>
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
    ///     handler::on,
    ///     service,
    ///     route,
    ///     routing::MethodFilter,
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
    /// let app = route("/", service::on(MethodFilter::DELETE, service));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn on<T>(self, method: MethodFilter, svc: T) -> OnMethod<T, Self, B>
    where
        T: Service<Request<B>> + Clone,
    {
        OnMethod {
            method,
            svc,
            fallback: self,
            _request_body: PhantomData,
        }
    }

    /// Handle errors this service might produce, by mapping them to responses.
    ///
    /// Unhandled errors will close the connection without sending a response.
    ///
    /// Works similarly to [`Router::handle_error`]. See that for more
    /// details.
    ///
    /// [`Router::handle_error`]: crate::routing::Router::handle_error
    pub fn handle_error<ReqBody, H>(self, f: H) -> HandleError<Self, H, ReqBody> {
        HandleError::new(self, f)
    }
}

// this is identical to `routing::OnMethod`'s implementation. Would be nice to find a way to clean
// that up, but not sure its possible.
impl<S, F, B, ResBody> Service<Request<B>> for OnMethod<S, F, B>
where
    S: Service<Request<B>, Response = Response<ResBody>> + Clone,
    ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    ResBody::Error: Into<BoxError>,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error> + Clone,
{
    type Response = Response<BoxBody>;
    type Error = S::Error;
    type Future = future::OnMethodFuture<S, F, B>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        use crate::util::Either;

        let req_method = req.method().clone();

        let f = if self.method.matches(req.method()) {
            let fut = self.svc.clone().oneshot(req);
            Either::A { inner: fut }
        } else {
            let fut = self.fallback.clone().oneshot(req);
            Either::B { inner: fut }
        };

        future::OnMethodFuture {
            inner: f,
            req_method,
        }
    }
}

/// A [`Service`] adapter that handles errors with a closure.
///
/// Created with
/// [`handler::Layered::handle_error`](crate::handler::Layered::handle_error) or
/// [`routing::Router::handle_error`](crate::routing::Router::handle_error).
/// See those methods for more details.
pub struct HandleError<S, F, B> {
    inner: S,
    f: F,
    _marker: PhantomData<fn() -> B>,
}

impl<S, F, B> Clone for HandleError<S, F, B>
where
    S: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.inner.clone(), self.f.clone())
    }
}

impl<S, F, B> HandleError<S, F, B> {
    pub(crate) fn new(inner: S, f: F) -> Self {
        Self {
            inner,
            f,
            _marker: PhantomData,
        }
    }
}

impl<S, F, B> fmt::Debug for HandleError<S, F, B>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandleError")
            .field("inner", &self.inner)
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

impl<S, F, ReqBody, ResBody, Res, E> Service<Request<ReqBody>> for HandleError<S, F, ReqBody>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone,
    F: FnOnce(S::Error) -> Result<Res, E> + Clone,
    Res: IntoResponse,
    ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    ResBody::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = E;
    type Future = future::HandleErrorFuture<Oneshot<S, Request<ReqBody>>, F>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        future::HandleErrorFuture {
            f: Some(self.f.clone()),
            inner: self.inner.clone().oneshot(req),
        }
    }
}
