use super::Handler;
use crate::response::Response;
use http::Request;
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower_service::Service;

/// An adapter that makes a [`Handler`] into a [`Service`].
///
/// Created with [`Handler::into_service`].
pub struct IntoService<H, S, T, B> {
    handler: H,
    state: S,
    _marker: PhantomData<fn() -> (T, B)>,
}

#[test]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<IntoService<(), (), NotSendSync, NotSendSync>>();
    assert_sync::<IntoService<(), (), NotSendSync, NotSendSync>>();
}

impl<H, S, T, B> IntoService<H, S, T, B> {
    pub(super) fn new(handler: H, state: S) -> Self {
        Self {
            handler,
            state,
            _marker: PhantomData,
        }
    }
}

impl<H, S, T, B> fmt::Debug for IntoService<H, S, T, B>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoService")
            .field("state", &self.state)
            .finish()
    }
}

impl<H, S, T, B> Clone for IntoService<H, S, T, B>
where
    H: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            state: self.state.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, S, T, B> Service<Request<B>> for IntoService<H, S, T, B>
where
    H: Handler<S, T, B> + Clone + Send + 'static,
    B: Send + 'static,
    S: Clone,
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
        let state = self.state.clone();
        let future = Handler::call(handler, state, req);
        let future = future.map(Ok as _);

        super::future::IntoServiceFuture::new(future)
    }
}
