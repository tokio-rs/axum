//! Error handling utilities
//!
//! See [error handling](../index.html#error-handling) for more details on how
//! error handling works in axum.

use crate::{
    body::{box_body, BoxBody},
    response::IntoResponse,
    BoxError,
};
use bytes::Bytes;
use futures_util::ready;
use http::{Request, Response};
use pin_project_lite::pin_project;
use std::convert::Infallible;
use std::{
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{util::Oneshot, ServiceExt as _};
use tower_layer::Layer;
use tower_service::Service;

/// [`Layer`] that applies [`HandleErrorLayer`] which is a [`Service`] adapter
/// that handles errors by converting into responses.
///
/// See [error handling](../index.html#error-handling) for more details.
pub struct HandleErrorLayer<F, B> {
    f: F,
    _marker: PhantomData<fn() -> B>,
}

impl<F, B> HandleErrorLayer<F, B> {
    /// Create a new `HandleErrorLayer`.
    pub fn new(f: F) -> Self {
        Self {
            f,
            _marker: PhantomData,
        }
    }
}

impl<F, B, S> Layer<S> for HandleErrorLayer<F, B>
where
    F: Clone,
{
    type Service = HandleError<S, F, B>;

    fn layer(&self, inner: S) -> Self::Service {
        HandleError {
            inner,
            f: self.f.clone(),
            _marker: PhantomData,
        }
    }
}

impl<F, B> Clone for HandleErrorLayer<F, B>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            _marker: PhantomData,
        }
    }
}

impl<F, B> fmt::Debug for HandleErrorLayer<F, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandleErrorLayer")
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

/// A [`Service`] adapter that handles errors by converting into responses.
///
/// See [error handling](../index.html#error-handling) for more details.
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
    /// Create a new `HandleError`.
    pub fn new(inner: S, f: F) -> Self {
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

impl<S, F, ReqBody, ResBody, Res> Service<Request<ReqBody>> for HandleError<S, F, ReqBody>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone,
    F: FnOnce(S::Error) -> Res + Clone,
    Res: IntoResponse,
    ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    ResBody::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = HandleErrorFuture<Oneshot<S, Request<ReqBody>>, F>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        HandleErrorFuture {
            f: Some(self.f.clone()),
            inner: self.inner.clone().oneshot(req),
        }
    }
}

/// Handle errors this service might produce, by mapping them to responses.
///
/// See [error handling](../index.html#error-handling) for more details.
pub trait HandleErrorExt<B>: Service<Request<B>> + Sized {
    #[allow(missing_docs)]
    fn handle_error<H>(self, f: H) -> HandleError<Self, H, B> {
        HandleError::new(self, f)
    }
}

impl<B, S> HandleErrorExt<B> for S where S: Service<Request<B>> {}

pin_project! {
    /// Response future for [`HandleError`](super::HandleError).
    #[derive(Debug)]
    pub struct HandleErrorFuture<Fut, F> {
        #[pin]
        pub(super) inner: Fut,
        pub(super) f: Option<F>,
    }
}

impl<Fut, F, E, B, Res> Future for HandleErrorFuture<Fut, F>
where
    Fut: Future<Output = Result<Response<B>, E>>,
    F: FnOnce(E) -> Res,
    Res: IntoResponse,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match ready!(this.inner.poll(cx)) {
            Ok(res) => Ok(res.map(box_body)).into(),
            Err(err) => {
                let f = this.f.take().unwrap();
                let res = f(err);
                Ok(res.into_response().map(box_body)).into()
            }
        }
    }
}

#[test]
fn traits() {
    use crate::tests::*;

    assert_send::<HandleError<(), (), NotSendSync>>();
    assert_sync::<HandleError<(), (), NotSendSync>>();
}
