//! Handler future types.

use crate::body::{box_body, BoxBody};
use futures_util::future::{BoxFuture, Either};
use http::{Method, Request, Response};
use http_body::Empty;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{util::Oneshot, Service};

pin_project! {
    /// The response future for [`OnMethod`](super::OnMethod).
    pub struct OnMethodFuture<F, B>
    where
        F: Service<Request<B>>
    {
        #[pin]
        pub(super) inner: Either<BoxFuture<'static, Result<Response<BoxBody>, F::Error>>, Oneshot<F, Request<B>>>,
        pub(super) req_method: Method,
    }
}

// TODO(david): impl debug for `OnMethodFuture`

impl<F, B> Future for OnMethodFuture<F, B>
where
    F: Service<Request<B>, Response = Response<BoxBody>>,
{
    type Output = Result<Response<BoxBody>, F::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let response = futures_util::ready!(this.inner.poll(cx))?;
        if this.req_method == &Method::HEAD {
            let response = response.map(|_| box_body(Empty::new()));
            Poll::Ready(Ok(response))
        } else {
            Poll::Ready(Ok(response))
        }
    }
}
