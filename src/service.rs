use crate::{
    body::{Body, BoxBody},
    response::IntoResponse,
    routing::{EmptyRouter, MethodFilter, OnMethod},
};
use bytes::Bytes;
use futures_util::ready;
use http::{Request, Response};
use pin_project::pin_project;
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{util::Oneshot, BoxError, Service, ServiceExt as _};

pub fn get<S>(svc: S) -> OnMethod<S, EmptyRouter> {
    on(MethodFilter::Get, svc)
}

pub fn post<S>(svc: S) -> OnMethod<S, EmptyRouter> {
    on(MethodFilter::Post, svc)
}

pub fn on<S>(method: MethodFilter, svc: S) -> OnMethod<S, EmptyRouter> {
    OnMethod {
        method,
        svc,
        fallback: EmptyRouter,
    }
}

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
