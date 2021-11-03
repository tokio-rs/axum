use crate::body::BoxBody;
use http::{Request, Response, StatusCode};
use std::{
    convert::Infallible,
    fmt,
    future::ready,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower_service::Service;

/// A [`Service`] that responds with `405 Method not allowed` to all requests.
///
/// This is used as the bottom service in a method router. You shouldn't have to
/// use it manually.
pub struct MethodNotAllowed<E = Infallible> {
    _marker: PhantomData<fn() -> E>,
}

impl<E> MethodNotAllowed<E> {
    pub(crate) fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<E> Clone for MethodNotAllowed<E> {
    fn clone(&self) -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<E> fmt::Debug for MethodNotAllowed<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MethodNotAllowed").finish()
    }
}

impl<B, E> Service<Request<B>> for MethodNotAllowed<E>
where
    B: Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = E;
    type Future = MethodNotAllowedFuture<E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Request<B>) -> Self::Future {
        let res = Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(crate::body::empty())
            .unwrap();

        MethodNotAllowedFuture::new(ready(Ok(res)))
    }
}

opaque_future! {
    /// Response future for [`MethodNotAllowed`](super::MethodNotAllowed).
    pub type MethodNotAllowedFuture<E> =
        std::future::Ready<Result<Response<BoxBody>, E>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traits() {
        use crate::test_helpers::*;

        assert_send::<MethodNotAllowed<NotSendSync>>();
        assert_sync::<MethodNotAllowed<NotSendSync>>();
    }
}
