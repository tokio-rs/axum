use super::Handler;
use crate::body::BoxBody;
use http::{Request, Response};
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
pub struct IntoService<H, B, T> {
    handler: H,
    _marker: PhantomData<fn() -> (B, T)>,
}

#[test]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<IntoService<(), NotSendSync, NotSendSync>>();
    assert_sync::<IntoService<(), NotSendSync, NotSendSync>>();
}

impl<H, B, T> IntoService<H, B, T> {
    pub(super) fn new(handler: H) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

impl<H, B, T> fmt::Debug for IntoService<H, B, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoService")
            .field(&format_args!("..."))
            .finish()
    }
}

impl<H, B, T> Clone for IntoService<H, B, T>
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

impl<H, T, B> Service<Request<B>> for IntoService<H, B, T>
where
    H: Handler<B, T> + Clone + Send + 'static,
    B: Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = super::future::IntoServiceFuture;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // `IntoService` can only be constructed from async functions which are always ready, or from
        // `Layered` which bufferes in `<Layered as Handler>::call` and is therefore also always
        // ready.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        use futures_util::future::FutureExt;

        let handler = self.handler.clone();
        let future = Handler::call(handler, req).map(Ok::<_, Infallible> as _);

        super::future::IntoServiceFuture::new(future)
    }
}
