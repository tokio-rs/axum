//! Handler future types.

use crate::body::{box_body, BoxBody};
use http::{Method, Request, Response};
use http_body::Empty;
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::Service;

opaque_future! {
    /// The response future for [`IntoService`](super::IntoService).
    pub type IntoServiceFuture =
        futures_util::future::BoxFuture<'static, Result<Response<BoxBody>, Infallible>>;
}

pin_project! {
    /// The response future for [`OnMethod`](super::OnMethod).
    #[derive(Debug)]
    pub struct OnMethodFuture<S, F, B>
    where
        S: Service<Request<B>>,
        F: Service<Request<B>>
    {
        #[pin]
        pub(super) inner: crate::routing::future::RouteFuture<S, F, B>,
        pub(super) req_method: Method,
    }
}

impl<S, F, B> Future for OnMethodFuture<S, F, B>
where
    S: Service<Request<B>, Response = Response<BoxBody>>,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error>,
{
    type Output = Result<Response<BoxBody>, S::Error>;

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
