use super::Router;
use std::{
    convert::Infallible,
    fmt,
    future::ready,
    task::{Context, Poll},
};
use tower_service::Service;

/// A [`MakeService`] that produces axum router services.
///
/// [`MakeService`]: tower::make::MakeService
pub struct IntoMakeService<B> {
    router: Router<B>,
}

impl<B> IntoMakeService<B> {
    pub(super) fn new(router: Router<B>) -> Self {
        Self { router }
    }
}

impl<B> Clone for IntoMakeService<B> {
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
        }
    }
}

impl<B> fmt::Debug for IntoMakeService<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoMakeService")
            .field("router", &self.router)
            .finish()
    }
}

impl<B, T> Service<T> for IntoMakeService<B> {
    type Response = Router<B>;
    type Error = Infallible;
    type Future = IntoMakeServiceFuture<B>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _target: T) -> Self::Future {
        IntoMakeServiceFuture::new(ready(Ok(self.router.clone())))
    }
}

opaque_future! {
    /// Response future for [`IntoMakeService`].
    pub type IntoMakeServiceFuture<B> =
        std::future::Ready<Result<Router<B>, Infallible>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Body;

    #[test]
    fn traits() {
        use crate::tests::*;

        assert_send::<IntoMakeService<Body>>();
    }
}
