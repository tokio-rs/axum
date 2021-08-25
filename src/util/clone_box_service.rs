use futures_util::future::BoxFuture;
use std::future::Future;
use std::task::{Context, Poll};
use tower::ServiceExt;
use tower_service::Service;

/// A boxed Service that implements Clone
///
/// Could probably upstream this to tower
pub(crate) struct CloneBoxService<T, U, E> {
    inner: Box<
        dyn CloneService<T, Response = U, Error = E, Future = BoxFuture<'static, Result<U, E>>>
            + Send,
    >,
}

impl<T, U, E> CloneBoxService<T, U, E> {
    pub(crate) fn new<S>(inner: S) -> Self
    where
        S: Service<T, Response = U, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        let inner = Box::new(inner.map_future(|f| Box::pin(f) as _));
        Self { inner }
    }
}

impl<T, U, E> Clone for CloneBoxService<T, U, E> {
    fn clone(&self) -> Self {
        Self {
            inner: dyn_clone::clone_box(&*self.inner),
        }
    }
}

impl<T, U, E> Service<T> for CloneBoxService<T, U, E> {
    type Response = U;
    type Error = E;
    type Future = BoxFuture<'static, Result<U, E>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        CloneService::poll_ready(&mut *self.inner, cx)
    }

    fn call(&mut self, req: T) -> Self::Future {
        CloneService::call(&mut *self.inner, req)
    }
}

trait CloneService<R>: dyn_clone::DynClone {
    type Response;
    type Error;
    type Future: Future<Output = Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    fn call(&mut self, req: R) -> Self::Future;
}

impl<R, T> CloneService<R> for T
where
    T: Service<R> + Clone,
{
    type Response = T::Response;
    type Error = T::Error;
    type Future = T::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Service::poll_ready(self, cx)
    }

    fn call(&mut self, req: R) -> Self::Future {
        Service::call(self, req)
    }
}
