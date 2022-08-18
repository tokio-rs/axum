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

pub(crate) struct IntoServiceStateInExtension<H, T, M, S, B> {
    handler: H,
    _marker: PhantomData<fn() -> (T, M, S, B)>,
}

#[test]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<IntoServiceStateInExtension<(), NotSendSync, NotSendSync, (), NotSendSync>>();
    assert_sync::<IntoServiceStateInExtension<(), NotSendSync, NotSendSync, (), NotSendSync>>();
}

impl<H, T, M, S, B> IntoServiceStateInExtension<H, T, M, S, B> {
    pub(crate) fn new(handler: H) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

impl<H, T, M, S, B> fmt::Debug for IntoServiceStateInExtension<H, T, M, S, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoServiceStateInExtension")
            .field(&format_args!("..."))
            .finish()
    }
}

impl<H, T, M, S, B> Clone for IntoServiceStateInExtension<H, T, M, S, B>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, T, M, S, B> Service<Request<B>> for IntoServiceStateInExtension<H, T, M, S, B>
where
    H: Handler<T, M, S, B> + Clone + Send + 'static,
    B: Send + 'static,
    S: Send + Sync + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = super::future::IntoServiceFuture<H::Future>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // `IntoServiceStateInExtension` can only be constructed from async functions which are always ready, or
        // from `Layered` which bufferes in `<Layered as Handler>::call` and is therefore
        // also always ready.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        use futures_util::future::FutureExt;

        let state = req
            .extensions_mut()
            .remove::<Arc<S>>()
            .expect("state extension missing. This is a bug in axum, please file an issue");

        let handler = self.handler.clone();
        let future = Handler::call(handler, state, req);
        let future = future.map(Ok as _);

        super::future::IntoServiceFuture::new(future)
    }
}
