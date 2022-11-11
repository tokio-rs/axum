use crate::{response::Response, Extension};
use axum_core::response::IntoResponse;
use http::{Request, StatusCode};
use std::{
    convert::Infallible,
    future::{ready, Ready},
    task::{Context, Poll},
};
use sync_wrapper::SyncWrapper;
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
    type Future = Ready<Result<Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let res = (
            StatusCode::NOT_FOUND,
            Extension(FromDefaultFallback(req.map(SyncWrapper::new))),
        )
            .into_response();
        ready(Ok(res))
    }
}

pub(super) struct FromDefaultFallback<B>(pub(super) Request<SyncWrapper<B>>);
