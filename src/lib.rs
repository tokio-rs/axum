use self::{
    body::Body,
    routing::{AlwaysNotFound, RouteAt},
};
use bytes::Bytes;
use futures_util::ready;
use http::Response;
use pin_project::pin_project;
use std::{
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

mod error;

#[cfg(test)]
mod tests;

pub use self::error::Error;

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
    poll_ready_error: Option<Error>,
}

impl<R> Clone for IntoService<R>
where
    R: Clone,
{
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            poll_ready_error: None,
        }
    }
}

impl<R, B, T> Service<T> for IntoService<R>
where
    R: Service<T, Response = Response<B>>,
    R::Error: Into<Error>,
    B: Default,
{
    type Response = Response<B>;
    type Error = Error;
    type Future = HandleErrorFuture<R::Future, B>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if let Err(err) = ready!(self.app.service_tree.poll_ready(cx)).map_err(Into::into) {
            self.poll_ready_error = Some(err);
        }

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: T) -> Self::Future {
        if let Some(poll_ready_error) = self.poll_ready_error.take() {
            match error::handle_error::<B>(poll_ready_error) {
                Ok(res) => {
                    return HandleErrorFuture(Kind::Response(Some(res)));
                }
                Err(err) => {
                    return HandleErrorFuture(Kind::Error(Some(err)));
                }
            }
        }
        HandleErrorFuture(Kind::Future(self.app.service_tree.call(req)))
    }
}

#[pin_project]
pub struct HandleErrorFuture<F, B>(#[pin] Kind<F, B>);

#[pin_project(project = KindProj)]
enum Kind<F, B> {
    Response(Option<Response<B>>),
    Error(Option<Error>),
    Future(#[pin] F),
}

impl<F, B, E> Future for HandleErrorFuture<F, B>
where
    F: Future<Output = Result<Response<B>, E>>,
    E: Into<Error>,
    B: Default,
{
    type Output = Result<Response<B>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().0.project() {
            KindProj::Response(res) => Poll::Ready(Ok(res.take().unwrap())),
            KindProj::Error(err) => Poll::Ready(Err(err.take().unwrap())),
            KindProj::Future(fut) => match ready!(fut.poll(cx)) {
                Ok(res) => Poll::Ready(Ok(res)),
                Err(err) => Poll::Ready(error::handle_error(err.into())),
            },
        }
    }
}
