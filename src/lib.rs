use self::{
    body::Body,
    routing::{AlwaysNotFound, RouteAt},
};
use bytes::Bytes;
use futures_util::ready;
use http::Response;
use pin_project::pin_project;
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::Service;

pub mod body;
pub mod extract;
pub mod handler;
pub mod response;
pub mod routing;

#[cfg(test)]
mod tests;

pub fn app() -> App<AlwaysNotFound> {
    App {
        service_tree: AlwaysNotFound(()),
    }
}

#[derive(Debug, Clone)]
pub struct App<R> {
    service_tree: R,
}

impl<R> App<R> {
    pub fn at(self, route_spec: &str) -> RouteAt<R> {
        self.at_bytes(Bytes::copy_from_slice(route_spec.as_bytes()))
    }

    fn at_bytes(self, route_spec: Bytes) -> RouteAt<R> {
        RouteAt {
            app: self,
            route_spec,
        }
    }
}

pub struct IntoService<R> {
    app: App<R>,
}

impl<R> Clone for IntoService<R>
where
    R: Clone,
{
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
        }
    }
}

impl<R, B, T> Service<T> for IntoService<R>
where
    R: Service<T, Response = Response<B>, Error = Infallible>,
    B: Default,
{
    type Response = Response<B>;
    type Error = Infallible;
    type Future = HandleErrorFuture<R::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match ready!(self.app.service_tree.poll_ready(cx)) {
            Ok(_) => Poll::Ready(Ok(())),
            Err(err) => match err {},
        }
    }

    fn call(&mut self, req: T) -> Self::Future {
        HandleErrorFuture(self.app.service_tree.call(req))
    }
}

#[pin_project]
pub struct HandleErrorFuture<F>(#[pin] F);

impl<F, B> Future for HandleErrorFuture<F>
where
    F: Future<Output = Result<Response<B>, Infallible>>,
    B: Default,
{
    type Output = Result<Response<B>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().0.poll(cx)
    }
}

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
#[error("{0}")]
pub struct BoxStdError(#[source] tower::BoxError);
