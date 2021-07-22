//! [`Service`](tower::Service) future types.

use crate::{
    body::{box_body, BoxBody},
    response::IntoResponse,
};
use bytes::Bytes;
use futures_util::ready;
use http::Response;
use pin_project::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::BoxError;

/// Response future for [`HandleError`](super::HandleError).
#[pin_project]
#[derive(Debug)]
pub struct HandleErrorFuture<Fut, F> {
    #[pin]
    pub(super) inner: Fut,
    pub(super) f: Option<F>,
}

impl<Fut, F, E, E2, B, Res> Future for HandleErrorFuture<Fut, F>
where
    Fut: Future<Output = Result<Response<B>, E>>,
    F: FnOnce(E) -> Result<Res, E2>,
    Res: IntoResponse,
    B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError> + Send + Sync + 'static,
{
    type Output = Result<Response<BoxBody>, E2>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match ready!(this.inner.poll(cx)) {
            Ok(res) => Ok(res.map(box_body)).into(),
            Err(err) => {
                let f = this.f.take().unwrap();
                match f(err) {
                    Ok(res) => Ok(res.into_response().map(box_body)).into(),
                    Err(err) => Err(err).into(),
                }
            }
        }
    }
}
