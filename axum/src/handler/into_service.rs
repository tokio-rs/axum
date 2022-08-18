use super::Handler;
use crate::response::Response;
use http::Request;
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    sync::Arc,
    task::{Context, Poll},
};
use tower_service::Service;

/// An adapter that makes a [`Handler`] into a [`Service`].
///
/// Created with [`HandlerWithoutStateExt::into_service`].
///
/// [`HandlerWithoutStateExt::into_service`]: super::HandlerWithoutStateExt::into_service
pub struct IntoService<H, T, S, B> {
    handler: H,
    state: Arc<S>,
    _marker: PhantomData<fn() -> (T, B)>,
}

impl<H, T, S, B> IntoService<H, T, S, B> {
    /// Get a reference to the state.
    pub fn state(&self) -> &S {
        &self.state
    }
}

#[test]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<IntoService<(), NotSendSync, (), NotSendSync>>();
    assert_sync::<IntoService<(), NotSendSync, (), NotSendSync>>();
}

impl<H, T, S, B> IntoService<H, T, S, B> {
    pub(super) fn new(handler: H, state: Arc<S>) -> Self {
        Self {
            handler,
            state,
            _marker: PhantomData,
        }
    }
}

impl<H, T, S, B> fmt::Debug for IntoService<H, T, S, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoService")
            .field(&format_args!("..."))
            .finish()
    }
}

impl<H, T, S, B> Clone for IntoService<H, T, S, B>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            state: Arc::clone(&self.state),
            _marker: PhantomData,
        }
    }
}

impl<H, T, S, B> Service<Request<B>> for IntoService<H, T, S, B>
where
    H: Handler<T, S, B> + Clone + Send + 'static,
    B: Send + 'static,
    S: Send + Sync,
{
    type Response = Response;
    type Error = Infallible;
    type Future = super::future::IntoServiceFuture<H::Future>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // `IntoService` can only be constructed from async functions which are always ready, or
        // from `Layered` which bufferes in `<Layered as Handler>::call` and is therefore
        // also always ready.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        use futures_util::future::FutureExt;

        let handler = self.handler.clone();
        let future = Handler::call(handler, Arc::clone(&self.state), req);
        let future = future.map(Ok as _);

        super::future::IntoServiceFuture::new(future)
    }
}
