//! Use Tower [`Service`]s to handl requests.
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
//! use axum::{service, handler, prelude::*};
//!
//! async fn handler(request: Request<Body>) { /* ... */ }
//!
//! let redirect_service = Redirect::<Body>::permanent("/new".parse().unwrap());
//!
//! let app = route("/old", service::get(redirect_service))
//!     .route("/new", handler::get(handler));
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! # Regarding backpressure and `Service::poll_ready`
//!
//! Generally routing to one of multiple services and backpressure doesn't mix
//! well. Ideally you would want ensure a service is ready to receive a request
//! before calling the it. However in order to know which service to call you
//! need the request...
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
//! It also means if `poll_ready` returns an error that error will be returned
//! in the response future from `call`, and _not_ from `poll_ready`. In that
//! case the underlying service will _not_ be discarded and will continue to be
//! used for future requests. Services that expect to be discarded if
//! `poll_ready` fails should _not_ be used with axum.
//!
//! One possible approach is to only apply backpressure sensitive middleware
//! around your entire app. This is possible because axum applications are
//! themselves services:
//!
//! ```rust
//! use axum::prelude::*;
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
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
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
    body::{box_body, BoxBody},
    response::IntoResponse,
    routing::{EmptyRouter, MethodFilter, RouteFuture},
};
use bytes::Bytes;
use futures_util::ready;
use http::{Request, Response};
use pin_project::pin_project;
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower::{util::Oneshot, BoxError, Service, ServiceExt as _};

pub mod future;

/// Route requests to the given service regardless of the HTTP method.
///
/// See [`get`] for an example.
pub fn any<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::Any, svc)
}

/// Route `CONNECT` requests to the given service.
///
/// See [`get`] for an example.
pub fn connect<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::Connect, svc)
}

/// Route `DELETE` requests to the given service.
///
/// See [`get`] for an example.
pub fn delete<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::Delete, svc)
}

/// Route `GET` requests to the given service.
///
/// # Example
///
/// ```rust
/// use axum::{service, prelude::*};
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
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn get<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::Get, svc)
}

/// Route `HEAD` requests to the given service.
///
/// See [`get`] for an example.
pub fn head<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::Head, svc)
}

/// Route `OPTIONS` requests to the given service.
///
/// See [`get`] for an example.
pub fn options<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::Options, svc)
}

/// Route `PATCH` requests to the given service.
///
/// See [`get`] for an example.
pub fn patch<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::Patch, svc)
}

/// Route `POST` requests to the given service.
///
/// See [`get`] for an example.
pub fn post<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::Post, svc)
}

/// Route `PUT` requests to the given service.
///
/// See [`get`] for an example.
pub fn put<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::Put, svc)
}

/// Route `TRACE` requests to the given service.
///
/// See [`get`] for an example.
pub fn trace<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::Trace, svc)
}

/// Route requests with the given method to the service.
///
/// # Example
///
/// ```rust
/// use axum::{handler::on, service, routing::MethodFilter, prelude::*};
/// use http::Response;
/// use std::convert::Infallible;
/// use hyper::Body;
///
/// let service = tower::service_fn(|request: Request<Body>| async {
///     Ok::<_, Infallible>(Response::new(Body::empty()))
/// });
///
/// // Requests to `POST /` will go to `service`.
/// let app = route("/", service::on(MethodFilter::Post, service));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn on<S, B>(
    method: MethodFilter,
    svc: S,
) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    OnMethod {
        method,
        svc: BoxResponseBody {
            inner: svc,
            _request_body: PhantomData,
        },
        fallback: EmptyRouter::new(),
    }
}

/// A [`Service`] that accepts requests based on a [`MethodFilter`] and allows
/// chaining additional services.
#[derive(Clone, Debug)]
pub struct OnMethod<S, F> {
    pub(crate) method: MethodFilter,
    pub(crate) svc: S,
    pub(crate) fallback: F,
}

impl<S, F> OnMethod<S, F> {
    /// Chain an additional service that will accept all requests regardless of
    /// its HTTP method.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn any<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::Any, svc)
    }

    /// Chain an additional service that will only accept `CONNECT` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn connect<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::Connect, svc)
    }

    /// Chain an additional service that will only accept `DELETE` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn delete<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::Delete, svc)
    }

    /// Chain an additional service that will only accept `GET` requests.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{handler::on, service, routing::MethodFilter, prelude::*};
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
    /// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn get<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::Get, svc)
    }

    /// Chain an additional service that will only accept `HEAD` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn head<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::Head, svc)
    }

    /// Chain an additional service that will only accept `OPTIONS` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn options<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::Options, svc)
    }

    /// Chain an additional service that will only accept `PATCH` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn patch<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::Patch, svc)
    }

    /// Chain an additional service that will only accept `POST` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn post<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::Post, svc)
    }

    /// Chain an additional service that will only accept `PUT` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn put<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::Put, svc)
    }

    /// Chain an additional service that will only accept `TRACE` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn trace<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::Trace, svc)
    }

    /// Chain an additional service that will accept requests matching the given
    /// `MethodFilter`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{handler::on, service, routing::MethodFilter, prelude::*};
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
    /// let app = route("/", service::on(MethodFilter::Delete, service));
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn on<T, B>(self, method: MethodFilter, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        OnMethod {
            method,
            svc: BoxResponseBody {
                inner: svc,
                _request_body: PhantomData,
            },
            fallback: self,
        }
    }
}

// this is identical to `routing::OnMethod`'s implementation. Would be nice to find a way to clean
// that up, but not sure its possible.
impl<S, F, B> Service<Request<B>> for OnMethod<S, F>
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

    fn call(&mut self, req: Request<B>) -> Self::Future {
        if self.method.matches(req.method()) {
            let fut = self.svc.clone().oneshot(req);
            RouteFuture::a(fut)
        } else {
            let fut = self.fallback.clone().oneshot(req);
            RouteFuture::b(fut)
        }
    }
}

/// A [`Service`] adapter that handles errors with a closure.
///
/// Created with
/// [`handler::Layered::handle_error`](crate::handler::Layered::handle_error) or
/// [`routing::Layered::handle_error`](crate::routing::Layered::handle_error).
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

impl<S, F, B> crate::routing::RoutingDsl for HandleError<S, F, B> {}

impl<S, F, B> crate::sealed::Sealed for HandleError<S, F, B> {}

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

/// Extension trait that adds additional methods to [`Service`].
pub trait ServiceExt<ReqBody, ResBody>:
    Service<Request<ReqBody>, Response = Response<ResBody>>
{
    /// Handle errors from a service.
    ///
    /// `handle_error` takes a closure that will map errors from the service
    /// into responses. The closure's return type must be `Result<T, E>` where
    /// `T` implements [`IntoIntoResponse`](crate::response::IntoResponse).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use axum::{service::{self, ServiceExt}, prelude::*};
    /// use http::{Response, StatusCode};
    /// use tower::{service_fn, BoxError};
    /// use std::convert::Infallible;
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
    ///         Ok::<_, Infallible>((
    ///             StatusCode::INTERNAL_SERVER_ERROR,
    ///             error.to_string(),
    ///         ))
    ///     }),
    /// );
    /// #
    /// # async {
    /// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// It works similarly to [`routing::Layered::handle_error`]. See that for more details.
    ///
    /// [`routing::Layered::handle_error`]: crate::routing::Layered::handle_error
    fn handle_error<F, Res, E>(self, f: F) -> HandleError<Self, F, ReqBody>
    where
        Self: Sized,
        F: FnOnce(Self::Error) -> Result<Res, E>,
        Res: IntoResponse,
        ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        ResBody::Error: Into<BoxError> + Send + Sync + 'static,
    {
        HandleError::new(self, f)
    }

    /// Check that your service cannot fail.
    ///
    /// That is its error type is [`Infallible`].
    fn check_infallible(self) -> Self
    where
        Self: Service<Request<ReqBody>, Response = Response<ResBody>, Error = Infallible> + Sized,
    {
        self
    }
}

impl<S, ReqBody, ResBody> ServiceExt<ReqBody, ResBody> for S where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>
{
}

/// A [`Service`] that boxes response bodies.
pub struct BoxResponseBody<S, B> {
    inner: S,
    _request_body: PhantomData<fn() -> B>,
}

impl<S, B> Clone for BoxResponseBody<S, B>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _request_body: PhantomData,
        }
    }
}

impl<S, B> fmt::Debug for BoxResponseBody<S, B>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxResponseBody")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for BoxResponseBody<S, ReqBody>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone,
    ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    ResBody::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = S::Error;
    type Future = BoxResponseBodyFuture<Oneshot<S, Request<ReqBody>>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let fut = self.inner.clone().oneshot(req);
        BoxResponseBodyFuture(fut)
    }
}

/// Response future for [`BoxResponseBody`].
#[pin_project]
#[derive(Debug)]
pub struct BoxResponseBodyFuture<F>(#[pin] F);

impl<F, B, E> Future for BoxResponseBodyFuture<F>
where
    F: Future<Output = Result<Response<B>, E>>,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Output = Result<Response<BoxBody>, E>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.project().0.poll(cx))?;
        let res = res.map(box_body);
        Poll::Ready(Ok(res))
    }
}
