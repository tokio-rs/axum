use crate::extract::connect_info::{Connected, WithConnectInfo};
use std::{
    convert::Infallible,
    future::ready,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// A [`MakeService`] that produces axum router services.
///
/// [`MakeService`]: tower::make::MakeService
#[derive(Debug, Clone)]
pub struct IntoMakeService<M> {
    make_svc: M,
}

impl<S> IntoMakeService<Shared<S>> {
    pub(crate) fn new(svc: S) -> Self {
        Self {
            make_svc: Shared(svc),
        }
    }
}

impl<M> IntoMakeService<M> {
    pub fn with_connect_info<C, Target>(self) -> IntoMakeService<WithConnectInfo<M, C>>
    where
        C: Connected<Target>,
    {
        self.layer(tower_layer::layer_fn(WithConnectInfo::new))
    }

    pub fn layer<L>(self, layer: L) -> IntoMakeService<L::Service>
    where
        L: Layer<M>,
    {
        IntoMakeService {
            make_svc: layer.layer(self.make_svc),
        }
    }
}

impl<M, T> Service<T> for IntoMakeService<M>
where
    M: Service<T>,
{
    type Response = M::Response;
    type Error = M::Error;
    type Future = M::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.make_svc.poll_ready(cx)
    }

    fn call(&mut self, target: T) -> Self::Future {
        self.make_svc.call(target)
    }
}

#[derive(Debug, Clone)]
pub struct Shared<S>(S);

impl<S, T> Service<T> for Shared<S>
where
    S: Clone,
{
    type Response = S;
    type Error = Infallible;
    type Future = SharedFuture<S>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _target: T) -> Self::Future {
        SharedFuture::new(ready(Ok(self.0.clone())))
    }
}

opaque_future! {
    /// Response future for [`Shared`].
    pub type SharedFuture<S> =
        std::future::Ready<Result<S, Infallible>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Body;

    #[test]
    fn traits() {
        use crate::test_helpers::*;

        assert_send::<IntoMakeService<Body>>();
    }
}
