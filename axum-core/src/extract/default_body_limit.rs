#![allow(missing_docs)]

use http::Request;
use std::task::Context;
use tower_layer::Layer;
use tower_service::Service;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DefaultBodyLimit;

impl DefaultBodyLimit {
    pub fn disable() -> Self {
        Self
    }
}

impl<S> Layer<S> for DefaultBodyLimit {
    type Service = DefaultBodyLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DefaultBodyLimitService { inner }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DefaultBodyLimitService<S> {
    pub(super) inner: S,
}

impl<B, S> Service<Request<B>> for DefaultBodyLimitService<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        req.extensions_mut().insert(DefaultBodyLimitDisabled);
        self.inner.call(req)
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct DefaultBodyLimitDisabled;
