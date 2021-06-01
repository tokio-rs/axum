use self::{
    body::Body,
    routing::{AlwaysNotFound, RouteAt},
};
use body::BoxBody;
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
use tower::{BoxError, Service};

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
    type Future = R::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match ready!(self.app.service_tree.poll_ready(cx)) {
            Ok(_) => Poll::Ready(Ok(())),
            Err(err) => match err {},
        }
    }

    fn call(&mut self, req: T) -> Self::Future {
        self.app.service_tree.call(req)
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
pub struct BoxStdError(#[source] pub(crate) tower::BoxError);

pub trait ServiceExt<B>: Service<Request<Body>, Response = Response<B>> {
    fn handle_error<F, NewBody>(self, f: F) -> HandleError<Self, F, Self::Error>
    where
        Self: Sized,
        F: FnOnce(Self::Error) -> Response<NewBody>,
        B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
        NewBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        NewBody::Error: Into<BoxError> + Send + Sync + 'static,
    {
        HandleError {
            inner: self,
            f,
            poll_ready_error: None,
        }
    }
}

impl<S, B> ServiceExt<B> for S where S: Service<Request<Body>, Response = Response<B>> {}

pub struct HandleError<S, F, E> {
    inner: S,
    f: F,
    poll_ready_error: Option<E>,
}

impl<S, F, E> fmt::Debug for HandleError<S, F, E>
where
    S: fmt::Debug,
    E: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandleError")
            .field("inner", &self.inner)
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .field("poll_ready_error", &self.poll_ready_error)
            .finish()
    }
}

impl<S, F, E> Clone for HandleError<S, F, E>
where
    S: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            f: self.f.clone(),
            poll_ready_error: None,
        }
    }
}

impl<S, F, B, NewBody> Service<Request<Body>> for HandleError<S, F, S::Error>
where
    S: Service<Request<Body>, Response = Response<B>>,
    F: FnOnce(S::Error) -> Response<NewBody> + Clone,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
    NewBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    NewBody::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = HandleErrorFuture<S::Future, F, S::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match ready!(self.inner.poll_ready(cx)) {
            Ok(_) => Poll::Ready(Ok(())),
            Err(err) => {
                self.poll_ready_error = Some(err);
                Poll::Ready(Ok(()))
            }
        }
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        if let Some(err) = self.poll_ready_error.take() {
            return HandleErrorFuture {
                f: Some(self.f.clone()),
                kind: Kind::Error(Some(err)),
            };
        }

        HandleErrorFuture {
            f: Some(self.f.clone()),
            kind: Kind::Future(self.inner.call(req)),
        }
    }
}

#[pin_project]
pub struct HandleErrorFuture<Fut, F, E> {
    #[pin]
    kind: Kind<Fut, E>,
    f: Option<F>,
}

#[pin_project(project = KindProj)]
enum Kind<Fut, E> {
    Future(#[pin] Fut),
    Error(Option<E>),
}

impl<Fut, F, E, B, NewBody> Future for HandleErrorFuture<Fut, F, E>
where
    Fut: Future<Output = Result<Response<B>, E>>,
    F: FnOnce(E) -> Response<NewBody>,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
    NewBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    NewBody::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.kind.project() {
            KindProj::Future(future) => match ready!(future.poll(cx)) {
                Ok(res) => Ok(res.map(BoxBody::new)).into(),
                Err(err) => {
                    let f = this.f.take().unwrap();
                    let res = f(err);
                    Ok(res.map(BoxBody::new)).into()
                }
            },
            KindProj::Error(err) => {
                let f = this.f.take().unwrap();
                let res = f(err.take().unwrap());
                Ok(res.map(BoxBody::new)).into()
            }
        }
    }
}
