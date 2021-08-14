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
//! use axum::{service, handler, prelude::*};
//!
//! async fn handler(request: Request<Body>) { /* ... */ }
//!
//! let redirect_service = Redirect::<Body>::permanent("/new".parse().unwrap());
//!
//! let app = route("/old", service::get(redirect_service))
//!     .route("/new", handler::get(handler));
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
    routing::{future::RouteFuture, EmptyRouter, MethodFilter},
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
pub fn any<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::all(), svc)
}

/// Route `CONNECT` requests to the given service.
///
/// See [`get`] for an example.
pub fn connect<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::CONNECT, svc)
}

/// Route `DELETE` requests to the given service.
///
/// See [`get`] for an example.
pub fn delete<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
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
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
pub fn get<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::GET, svc)
}

/// Route `HEAD` requests to the given service.
///
/// See [`get`] for an example.
pub fn head<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::HEAD, svc)
}

/// Route `OPTIONS` requests to the given service.
///
/// See [`get`] for an example.
pub fn options<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::OPTIONS, svc)
}

/// Route `PATCH` requests to the given service.
///
/// See [`get`] for an example.
pub fn patch<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::PATCH, svc)
}

/// Route `POST` requests to the given service.
///
/// See [`get`] for an example.
pub fn post<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::POST, svc)
}

/// Route `PUT` requests to the given service.
///
/// See [`get`] for an example.
pub fn put<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
where
    S: Service<Request<B>> + Clone,
{
    on(MethodFilter::PUT, svc)
}

/// Route `TRACE` requests to the given service.
///
/// See [`get`] for an example.
pub fn trace<S, B>(svc: S) -> OnMethod<BoxResponseBody<S, B>, EmptyRouter<S::Error>>
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
/// let app = route("/", service::on(MethodFilter::POST, service));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
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
        fallback: EmptyRouter::method_not_allowed(),
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
        self.on(MethodFilter::all(), svc)
    }

    /// Chain an additional service that will only accept `CONNECT` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn connect<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::CONNECT, svc)
    }

    /// Chain an additional service that will only accept `DELETE` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn delete<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
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
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    pub fn get<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::GET, svc)
    }

    /// Chain an additional service that will only accept `HEAD` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn head<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::HEAD, svc)
    }

    /// Chain an additional service that will only accept `OPTIONS` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn options<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::OPTIONS, svc)
    }

    /// Chain an additional service that will only accept `PATCH` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn patch<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::PATCH, svc)
    }

    /// Chain an additional service that will only accept `POST` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn post<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::POST, svc)
    }

    /// Chain an additional service that will only accept `PUT` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn put<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
    where
        T: Service<Request<B>> + Clone,
    {
        self.on(MethodFilter::PUT, svc)
    }

    /// Chain an additional service that will only accept `TRACE` requests.
    ///
    /// See [`OnMethod::get`] for an example.
    pub fn trace<T, B>(self, svc: T) -> OnMethod<BoxResponseBody<T, B>, Self>
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
    /// let app = route("/", service::on(MethodFilter::DELETE, service));
    /// # async {
    /// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
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

    /// Handle errors this service might produce, by mapping them to responses.
    ///
    /// Unhandled errors will close the connection without sending a response.
    ///
    /// Works similarly to [`RoutingDsl::handle_error`]. See that for more
    /// details.
    ///
    /// [`RoutingDsl::handle_error`]: crate::routing::RoutingDsl::handle_error
    pub fn handle_error<ReqBody, H, Res, E>(
        self,
        f: H,
    ) -> HandleError<Self, H, ReqBody, HandleErrorFromService>
    where
        Self: Service<Request<ReqBody>, Response = Response<BoxBody>>,
        H: FnOnce(<Self as Service<Request<ReqBody>>>::Error) -> Result<Res, E>,
        Res: IntoResponse,
    {
        HandleError::new(self, f)
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
    type Future = future::OnMethodFuture<S, F, B>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let f = if self.method.matches(req.method()) {
            let fut = self.svc.clone().oneshot(req);
            RouteFuture::a(fut)
        } else {
            let fut = self.fallback.clone().oneshot(req);
            RouteFuture::b(fut)
        };

        future::OnMethodFuture { inner: f }
    }
}

/// A [`Service`] adapter that handles errors with a closure.
///
/// Created with
/// [`handler::Layered::handle_error`](crate::handler::Layered::handle_error) or
/// [`routing::RoutingDsl::handle_error`](crate::routing::RoutingDsl::handle_error).
/// See those methods for more details.
pub struct HandleError<S, F, B, T> {
    inner: S,
    f: F,
    _marker: PhantomData<fn() -> (B, T)>,
}

impl<S, F, B, T> Clone for HandleError<S, F, B, T>
where
    S: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.inner.clone(), self.f.clone())
    }
}

/// Marker type used for [`HandleError`] to indicate that it should implement
/// [`RoutingDsl`](crate::routing::RoutingDsl).
#[non_exhaustive]
#[derive(Debug)]
pub struct HandleErrorFromRouter;

/// Marker type used for [`HandleError`] to indicate that it should _not_ implement
/// [`RoutingDsl`](crate::routing::RoutingDsl).
#[non_exhaustive]
#[derive(Debug)]
pub struct HandleErrorFromService;

impl<S, F, B> crate::routing::RoutingDsl for HandleError<S, F, B, HandleErrorFromRouter> {}

impl<S, F, B> crate::sealed::Sealed for HandleError<S, F, B, HandleErrorFromRouter> {}

impl<S, F, B, T> HandleError<S, F, B, T> {
    pub(crate) fn new(inner: S, f: F) -> Self {
        Self {
            inner,
            f,
            _marker: PhantomData,
        }
    }
}

impl<S, F, B, T> fmt::Debug for HandleError<S, F, B, T>
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

impl<S, F, ReqBody, ResBody, Res, E, T> Service<Request<ReqBody>> for HandleError<S, F, ReqBody, T>
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
    type Future = future::BoxResponseBodyFuture<Oneshot<S, Request<ReqBody>>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let fut = self.inner.clone().oneshot(req);
        future::BoxResponseBodyFuture { future: fut }
    }
}

/// ```compile_fail
/// use crate::{service::ServiceExt, prelude::*};
/// use tower::service_fn;
/// use hyper::Body;
/// use http::{Request, Response, StatusCode};
///
/// let svc = service_fn(|_: Request<Body>| async {
///     Ok::<_, hyper::Error>(Response::new(Body::empty()))
/// })
/// .handle_error::<_, _, hyper::Error>(|_| Ok(StatusCode::INTERNAL_SERVER_ERROR));
///
/// // `.route` should not compile, ie `HandleError` created from any
/// // random service should not implement `RoutingDsl`
/// svc.route::<_, Body>("/", get(|| async {}));
/// ```
#[allow(dead_code)]
fn compile_fail_tests() {}
