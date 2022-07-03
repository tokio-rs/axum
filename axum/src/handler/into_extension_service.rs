use super::Handler;
use crate::{response::Response, util::extract_state_assume_present};
use http::Request;
use std::{
    convert::Infallible,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower_service::Service;

/// A `Handler` converted into a `Service` that reads the state from request extensions. Panics if
/// the state is missing.
pub(crate) struct IntoExtensionService<H, S, T, B> {
    handler: H,
    _marker: PhantomData<fn() -> (S, T, B)>,
}

impl<H, S, T, B> IntoExtensionService<H, S, T, B> {
    pub(crate) fn new(handler: H) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

impl<H, S, T, B> Clone for IntoExtensionService<H, S, T, B>
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

impl<H, S, T, B> Service<Request<B>> for IntoExtensionService<H, S, T, B>
where
    H: Handler<S, T, B> + Clone + Send + 'static,
    B: Send + 'static,
    S: Clone + Send + Sync + 'static,
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

        let state = extract_state_assume_present::<S, _>(&req);
        let future = Handler::call(handler, state, req);
        let future = future.map(Ok as _);

        super::future::IntoServiceFuture::new(future)
    }
}
