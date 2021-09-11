use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use sync_wrapper::SyncWrapper;

use futures_util::future::BoxFuture;

use tower_layer::{layer_fn, LayerFn};
use tower_service::Service;

trait CloneService<Request>: Service<Request> {
    fn clone_box(
        &self,
    ) -> Box<
        dyn CloneService<
                Request,
                Response = Self::Response,
                Error = Self::Error,
                Future = Self::Future,
            > + Send
            + Sync,
    >;
}

impl<Request, T> CloneService<Request> for T
where
    T: Service<Request> + Send + Sync + Clone + 'static,
{
    fn clone_box(
        &self,
    ) -> Box<
        dyn CloneService<Request, Response = T::Response, Error = T::Error, Future = T::Future>
            + Send
            + Sync,
    > {
        Box::new(self.clone())
    }
}

pub(crate) struct BoxService<T, U, E> {
    inner: Box<
        dyn CloneService<T, Response = U, Error = E, Future = BoxServiceFuture<U, E>> + Send + Sync,
    >,
}

impl<T, U, E> BoxService<T, U, E> {
    pub(crate) fn new<S>(inner: S) -> Self
    where
        S: Service<T, Response = U, Error = E> + Clone + Send + Sync + 'static,
        S::Future: Send + 'static,
    {
        let inner = Box::new(Boxed { inner });
        BoxService { inner }
    }

    pub(crate) fn layer<S>() -> LayerFn<fn(S) -> Self>
    where
        S: Service<T, Response = U, Error = E> + Clone + Send + Sync + 'static,
        S::Future: Send + 'static,
    {
        layer_fn(Self::new)
    }
}

impl<T, U, E> Service<T> for BoxService<T, U, E> {
    type Response = U;
    type Error = E;
    type Future = BoxServiceFuture<U, E>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), E>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: T) -> Self::Future {
        self.inner.call(request)
    }
}

impl<T, U, E> Clone for BoxService<T, U, E> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone_box(),
        }
    }
}

#[derive(Clone)]
struct Boxed<S> {
    inner: S,
}

impl<S, Request> Service<Request> for Boxed<S>
where
    S: Service<Request> + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxServiceFuture<S::Response, S::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        BoxServiceFuture::new(self.inner.call(request))
    }
}

pub(crate) struct BoxServiceFuture<R, E> {
    fut: SyncWrapper<BoxFuture<'static, Result<R, E>>>,
}

impl<R, E> BoxServiceFuture<R, E> {
    fn new<F>(fut: F) -> Self
    where
        F: Future<Output = Result<R, E>> + Send + 'static,
    {
        Self {
            fut: SyncWrapper::new(Box::pin(fut)),
        }
    }
}

impl<R, E> Future for BoxServiceFuture<R, E> {
    type Output = Result<R, E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.fut.get_mut().as_mut().poll(cx)
    }
}
