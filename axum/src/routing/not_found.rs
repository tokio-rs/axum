use crate::response::Response;
use http::{Request, StatusCode};
use std::{
    convert::Infallible,
    future::ready,
    task::{Context, Poll},
};
use tower_service::Service;

/// A [`Service`] that responds with `404 Not Found` to all requests.
///
/// This is used as the bottom service in a method router. You shouldn't have to
/// use it manually.
#[derive(Clone, Copy, Debug)]
pub(super) struct NotFound;

impl<B> Service<Request<B>> for NotFound
where
    B: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = std::future::Ready<Result<Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Request<B>) -> Self::Future {
        let res = Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(crate::body::empty())
            .unwrap();

        ready(Ok(res))
    }
}
