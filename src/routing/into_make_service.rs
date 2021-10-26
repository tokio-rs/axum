use std::{
    convert::Infallible,
    future::ready,
    task::{Context, Poll},
};
use tower_service::Service;

/// A [`MakeService`] that produces axum router services.
///
/// [`MakeService`]: tower::make::MakeService
#[derive(Debug, Clone)]
pub struct IntoMakeService<S> {
    service: S,
}

impl<S> IntoMakeService<S> {
    pub(super) fn new(service: S) -> Self {
        Self { service }
    }
}

impl<S, T> Service<T> for IntoMakeService<S>
where
    S: Clone,
{
    type Response = S;
    type Error = Infallible;
    type Future = MakeRouteServiceFuture<S>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _target: T) -> Self::Future {
        MakeRouteServiceFuture::new(ready(Ok(self.service.clone())))
    }
}

opaque_future! {
    /// Response future from [`MakeRouteService`] services.
    pub type MakeRouteServiceFuture<S> =
        std::future::Ready<Result<S, Infallible>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traits() {
        use crate::tests::*;

        assert_send::<IntoMakeService<()>>();
        assert_sync::<IntoMakeService<()>>();
    }
}
