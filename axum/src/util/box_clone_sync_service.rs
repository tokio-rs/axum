use futures_util::future::BoxFuture;
use std::{
    fmt,
    task::{Context, Poll},
};
use tower::ServiceExt;
use tower_service::Service;

pub(crate) struct BoxCloneSyncService<T, U, E>(
    Box<
        dyn CloneSyncService<T, Response = U, Error = E, Future = BoxFuture<'static, Result<U, E>>>
            + Send
            + Sync,
    >,
);

impl<T, U, E> BoxCloneSyncService<T, U, E> {
    pub(crate) fn new<S>(inner: S) -> Self
    where
        S: Service<T, Response = U, Error = E> + Clone + Send + Sync + 'static,
        S::Future: Send + 'static,
    {
        let inner = inner.map_future(|f| Box::pin(f) as _);
        BoxCloneSyncService(Box::new(inner))
    }
}

impl<T, U, E> Service<T> for BoxCloneSyncService<T, U, E> {
    type Response = U;
    type Error = E;
    type Future = BoxFuture<'static, Result<U, E>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), E>> {
        self.0.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, request: T) -> Self::Future {
        self.0.call(request)
    }
}

impl<T, U, E> Clone for BoxCloneSyncService<T, U, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

trait CloneSyncService<R>: Service<R> {
    fn clone_box(
        &self,
    ) -> Box<
        dyn CloneSyncService<
                R,
                Response = Self::Response,
                Error = Self::Error,
                Future = Self::Future,
            > + Send
            + Sync,
    >;
}

impl<R, T> CloneSyncService<R> for T
where
    T: Service<R> + Send + Sync + Clone + 'static,
{
    fn clone_box(
        &self,
    ) -> Box<
        dyn CloneSyncService<R, Response = T::Response, Error = T::Error, Future = T::Future>
            + Send
            + Sync,
    > {
        Box::new(self.clone())
    }
}

impl<T, U, E> fmt::Debug for BoxCloneSyncService<T, U, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BoxCloneSyncService").finish()
    }
}
