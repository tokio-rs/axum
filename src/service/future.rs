//! [`Service`](tower::Service) future types.

use crate::{
    body::{box_body, BoxBody},
    util::{Either, EitherProj},
    BoxError,
};
use bytes::Bytes;
use futures_util::ready;
use http::{Method, Request, Response};
use http_body::Empty;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::util::Oneshot;
use tower_service::Service;

pin_project! {
    /// The response future for [`OnMethod`](super::OnMethod).
    pub struct OnMethodFuture<S, F, B>
    where
        S: Service<Request<B>>,
        F: Service<Request<B>>
    {
        #[pin]
        pub(super) inner: Either<
            Oneshot<S, Request<B>>,
            Oneshot<F, Request<B>>,
        >,
        // pub(super) inner: crate::routing::future::RouteFuture<S, F, B>,
        pub(super) req_method: Method,
    }
}

impl<S, F, B, ResBody> Future for OnMethodFuture<S, F, B>
where
    S: Service<Request<B>, Response = Response<ResBody>> + Clone,
    ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    ResBody::Error: Into<BoxError>,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error>,
{
    type Output = Result<Response<BoxBody>, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let response = match this.inner.project() {
            EitherProj::A { inner } => ready!(inner.poll(cx))?.map(box_body),
            EitherProj::B { inner } => ready!(inner.poll(cx))?,
        };

        if this.req_method == &Method::HEAD {
            let response = response.map(|_| box_body(Empty::new()));
            Poll::Ready(Ok(response))
        } else {
            Poll::Ready(Ok(response))
        }
    }
}
