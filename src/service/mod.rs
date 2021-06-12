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
//! use tower_web::{service, handler, prelude::*};
//!
//! async fn handler(request: Request<Body>) { /* ... */ }
//!
//! let redirect_service = Redirect::<Body>::permanent("/new".parse().unwrap());
//!
//! let app = route("/old", service::get(redirect_service))
//!     .route("/new", handler::get(handler));
//! # async {
//! # app.serve(&"".parse().unwrap()).await.unwrap();
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
//! tower-web expects that all services used in your app wont care about
//! backpressure and so it uses the latter strategy. However that means you
//! should avoid routing to a service (or using a middleware) that _does_ care
//! about backpressure. At the very least you should [load shed] so requests are
//! dropped quickly and don't keep piling up.
//!
//! It also means if `poll_ready` returns an error that error will be returned
//! in the response future from `call`, and _not_ from `poll_ready`. In that
//! case the underlying service will _not_ be discarded and will continue to be
//! used for future requests. Services that expect to be discarded if
//! `poll_ready` fails should _not_ be used with tower-web.
//!
//! One possible approach is to only apply backpressure sensitive middleware
//! around your entire app. This is possible because tower-web applications are
//! themselves services:
//!
//! ```rust
//! use tower_web::prelude::*;
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
    body::{self, Body, BoxBody},
    response::IntoResponse,
    routing::{EmptyRouter, MethodFilter, RouteFuture},
};
use bytes::Bytes;
use http::{Request, Response};
use std::{
    convert::Infallible,
    fmt,
    task::{Context, Poll},
};
use tower::{util::Oneshot, BoxError, Service, ServiceExt as _};

pub mod future;

/// Route `CONNECT` requests to the given service.
///
/// See [`get`] for an example.
pub fn connect<S>(svc: S) -> OnMethod<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    on(MethodFilter::Connect, svc)
}

/// Route `DELETE` requests to the given service.
///
/// See [`get`] for an example.
pub fn delete<S>(svc: S) -> OnMethod<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    on(MethodFilter::Delete, svc)
}

/// Route `GET` requests to the given service.
///
/// # Example
///
/// ```rust
/// use tower_web::{service, prelude::*};
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
/// ```
///
/// You can only add services who cannot fail (their error type must be
/// [`Infallible`]). To gracefully handle errors see [`ServiceExt::handle_error`].
pub fn get<S>(svc: S) -> OnMethod<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    on(MethodFilter::Get, svc)
}

/// Route `HEAD` requests to the given service.
///
/// See [`get`] for an example.
pub fn head<S>(svc: S) -> OnMethod<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    on(MethodFilter::Head, svc)
}

/// Route `OPTIONS` requests to the given service.
///
/// See [`get`] for an example.
pub fn options<S>(svc: S) -> OnMethod<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    on(MethodFilter::Options, svc)
}

/// Route `PATCH` requests to the given service.
///
/// See [`get`] for an example.
pub fn patch<S>(svc: S) -> OnMethod<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    on(MethodFilter::Patch, svc)
}

/// Route `POST` requests to the given service.
///
/// See [`get`] for an example.
pub fn post<S>(svc: S) -> OnMethod<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    on(MethodFilter::Post, svc)
}

/// Route `PUT` requests to the given service.
///
/// See [`get`] for an example.
pub fn put<S>(svc: S) -> OnMethod<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    on(MethodFilter::Put, svc)
}

/// Route `TRACE` requests to the given service.
///
/// See [`get`] for an example.
pub fn trace<S>(svc: S) -> OnMethod<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    on(MethodFilter::Trace, svc)
}

/// Route requests with the given method to the service.
///
/// # Example
///
/// ```rust
/// use tower_web::{handler::on, service, routing::MethodFilter, prelude::*};
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
/// ```
pub fn on<S>(method: MethodFilter, svc: S) -> OnMethod<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    OnMethod {
        method,
        svc,
        fallback: EmptyRouter,
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
    pub fn any<T>(self, svc: T) -> OnMethod<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        self.on(MethodFilter::Any, svc)
    }

    /// Chain an additional service that will only accept `CONNECT` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn connect<T>(self, svc: T) -> OnMethod<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        self.on(MethodFilter::Connect, svc)
    }

    /// Chain an additional service that will only accept `DELETE` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn delete<T>(self, svc: T) -> OnMethod<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        self.on(MethodFilter::Delete, svc)
    }

    /// Chain an additional service that will only accept `GET` requests.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tower_web::{handler::on, service, routing::MethodFilter, prelude::*};
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
    /// ```
    ///
    /// You can only add services who cannot fail (their error type must be
    /// [`Infallible`]). To gracefully handle errors see
    /// [`ServiceExt::handle_error`].
    pub fn get<T>(self, svc: T) -> OnMethod<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        self.on(MethodFilter::Get, svc)
    }

    /// Chain an additional service that will only accept `HEAD` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn head<T>(self, svc: T) -> OnMethod<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        self.on(MethodFilter::Head, svc)
    }

    /// Chain an additional service that will only accept `OPTIONS` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn options<T>(self, svc: T) -> OnMethod<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        self.on(MethodFilter::Options, svc)
    }

    /// Chain an additional service that will only accept `PATCH` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn patch<T>(self, svc: T) -> OnMethod<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        self.on(MethodFilter::Patch, svc)
    }

    /// Chain an additional service that will only accept `POST` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn post<T>(self, svc: T) -> OnMethod<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        self.on(MethodFilter::Post, svc)
    }

    /// Chain an additional service that will only accept `PUT` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn put<T>(self, svc: T) -> OnMethod<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        self.on(MethodFilter::Put, svc)
    }

    /// Chain an additional service that will only accept `TRACE` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn trace<T>(self, svc: T) -> OnMethod<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        self.on(MethodFilter::Trace, svc)
    }

    /// Chain an additional service that will accept requests matching the given
    /// `MethodFilter`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tower_web::{handler::on, service, routing::MethodFilter, prelude::*};
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
    /// ```
    pub fn on<T>(self, method: MethodFilter, svc: T) -> OnMethod<T, Self>
    where
        T: Service<Request<Body>, Error = Infallible> + Clone,
    {
        OnMethod {
            method,
            svc,
            fallback: self,
        }
    }
}

// this is identical to `routing::OnMethod`'s implementation. Would be nice to find a way to clean
// that up, but not sure its possible.
impl<S, F, SB, FB> Service<Request<Body>> for OnMethod<S, F>
where
    S: Service<Request<Body>, Response = Response<SB>, Error = Infallible> + Clone,
    F: Service<Request<Body>, Response = Response<FB>, Error = Infallible> + Clone,

    SB: http_body::Body<Data = Bytes>,
    SB::Error: Into<BoxError>,
    FB: http_body::Body<Data = Bytes>,
    FB::Error: Into<BoxError>,
{
    type Response = Response<body::Or<SB, FB>>;
    type Error = Infallible;
    type Future = RouteFuture<S, F>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
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
#[derive(Clone)]
pub struct HandleError<S, F> {
    pub(crate) inner: S,
    pub(crate) f: F,
}

impl<S, F> crate::routing::RoutingDsl for HandleError<S, F> {}

impl<S, F> crate::sealed::Sealed for HandleError<S, F> {}

impl<S, F> HandleError<S, F> {
    pub(crate) fn new(inner: S, f: F) -> Self {
        Self { inner, f }
    }
}

impl<S, F> fmt::Debug for HandleError<S, F>
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

impl<S, F, B, Res> Service<Request<Body>> for HandleError<S, F>
where
    S: Service<Request<Body>, Response = Response<B>> + Clone,
    F: FnOnce(S::Error) -> Res + Clone,
    Res: IntoResponse,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = future::HandleErrorFuture<Oneshot<S, Request<Body>>, F>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        future::HandleErrorFuture {
            f: Some(self.f.clone()),
            inner: self.inner.clone().oneshot(req),
        }
    }
}

/// Extension trait that adds additional methods to [`Service`].
pub trait ServiceExt<B>: Service<Request<Body>, Response = Response<B>> {
    /// Handle errors from a service.
    ///
    /// tower-web requires all handlers and services, that are part of the
    /// router, to never return errors. If you route to [`Service`], not created
    /// by tower-web, who's error isn't `Infallible` you can use this combinator
    /// to handle the error.
    ///
    /// `handle_error` takes a closure that will map errors from the service
    /// into responses. The closure's return type must implement
    /// [`IntoResponse`].
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use tower_web::{service::{self, ServiceExt}, prelude::*};
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
    /// # app.serve(&"".parse().unwrap()).await.unwrap();
    /// # };
    /// ```
    fn handle_error<F, Res>(self, f: F) -> HandleError<Self, F>
    where
        Self: Sized,
        F: FnOnce(Self::Error) -> Res,
        Res: IntoResponse,
        B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
    {
        HandleError::new(self, f)
    }
}

impl<S, B> ServiceExt<B> for S where S: Service<Request<Body>, Response = Response<B>> {}
