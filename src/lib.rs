use self::body::Body;
use body::BoxBody;
use bytes::Bytes;
use futures_util::ready;
use handler::HandlerSvc;
use http::{Method, Request, Response};
use pin_project::pin_project;
use response::IntoResponse;
use routing::{EmptyRouter, OnMethod, Route};
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{util::Oneshot, BoxError, Service, ServiceExt as _};

pub mod body;
pub mod extract;
pub mod handler;
pub mod response;
pub mod routing;

#[doc(inline)]
pub use self::handler::Handler;
#[doc(inline)]
pub use self::routing::AddRoute;

pub use async_trait::async_trait;
pub use tower_http::add_extension::{AddExtension, AddExtensionLayer};

#[derive(Debug, Copy, Clone)]
pub enum MethodFilter {
    Any,
    Connect,
    Delete,
    Get,
    Head,
    Options,
    Patch,
    Post,
    Put,
    Trace,
}

impl MethodFilter {
    #[allow(clippy::match_like_matches_macro)]
    fn matches(self, method: &Method) -> bool {
        use MethodFilter::*;

        match (self, method) {
            (Any, _)
            | (Connect, &Method::CONNECT)
            | (Delete, &Method::DELETE)
            | (Get, &Method::GET)
            | (Head, &Method::HEAD)
            | (Options, &Method::OPTIONS)
            | (Patch, &Method::PATCH)
            | (Post, &Method::POST)
            | (Put, &Method::PUT)
            | (Trace, &Method::TRACE) => true,
            _ => false,
        }
    }
}

pub fn route<S>(spec: &str, svc: S) -> Route<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    routing::EmptyRouter.route(spec, svc)
}

pub fn get<H, B, T>(handler: H) -> OnMethod<HandlerSvc<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on_method(MethodFilter::Get, HandlerSvc::new(handler))
}

pub fn post<H, B, T>(handler: H) -> OnMethod<HandlerSvc<H, B, T>, EmptyRouter>
where
    H: Handler<B, T>,
{
    on_method(MethodFilter::Post, HandlerSvc::new(handler))
}

pub fn on_method<S>(method: MethodFilter, svc: S) -> OnMethod<S, EmptyRouter> {
    OnMethod {
        method,
        svc,
        fallback: EmptyRouter,
    }
}

#[cfg(test)]
mod tests;

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

// work around for `BoxError` not implementing `std::error::Error`
//
// This is currently required since tower-http's Compression middleware's body type's
// error only implements error when the inner error type does:
// https://github.com/tower-rs/tower-http/blob/master/tower-http/src/lib.rs#L310
//
// Fixing that is a breaking change to tower-http so we should wait a bit, but should
// totally fix it at some point.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct BoxStdError(#[from] pub(crate) tower::BoxError);

pub trait ServiceExt<B>: Service<Request<Body>, Response = Response<B>> {
    fn handle_error<F, Res>(self, f: F) -> HandleError<Self, F>
    where
        Self: Sized,
        F: FnOnce(Self::Error) -> Res,
        Res: IntoResponse<Body>,
        B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
    {
        HandleError::new(self, f)
    }
}

impl<S, B> ServiceExt<B> for S where S: Service<Request<Body>, Response = Response<B>> {}

#[derive(Clone)]
pub struct HandleError<S, F> {
    inner: S,
    f: F,
}

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
    Res: IntoResponse<Body>,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = HandleErrorFuture<Oneshot<S, Request<Body>>, F>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        HandleErrorFuture {
            f: Some(self.f.clone()),
            inner: self.inner.clone().oneshot(req),
        }
    }
}

#[pin_project]
pub struct HandleErrorFuture<Fut, F> {
    #[pin]
    inner: Fut,
    f: Option<F>,
}

impl<Fut, F, E, B, Res> Future for HandleErrorFuture<Fut, F>
where
    Fut: Future<Output = Result<Response<B>, E>>,
    F: FnOnce(E) -> Res,
    Res: IntoResponse<Body>,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match ready!(this.inner.poll(cx)) {
            Ok(res) => Ok(res.map(BoxBody::new)).into(),
            Err(err) => {
                let f = this.f.take().unwrap();
                let res = f(err).into_response();
                Ok(res.map(BoxBody::new)).into()
            }
        }
    }
}
