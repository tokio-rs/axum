//! Future types.

use crate::body::BoxBody;
use http::{Request, Response};
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::util::Oneshot;
use tower_service::Service;

opaque_future! {
    /// Response future for [`Router`](super::Router).
    pub type RouterFuture<B> =
        futures_util::future::Either<
            Oneshot<super::Route<B>, Request<B>>,
            std::future::Ready<Result<Response<BoxBody>, Infallible>>,
        >;
}

impl<B> RouterFuture<B> {
    pub(super) fn from_oneshot(future: Oneshot<super::Route<B>, Request<B>>) -> Self {
        Self {
            future: futures_util::future::Either::Left(future),
        }
    }

    pub(super) fn from_response(response: Response<BoxBody>) -> Self {
        RouterFuture {
            future: futures_util::future::Either::Right(std::future::ready(Ok(response))),
        }
    }
}

opaque_future! {
    /// Response future for [`Route`](super::Route).
    pub type RouteFuture =
        futures_util::future::BoxFuture<'static, Result<Response<BoxBody>, Infallible>>;
}

opaque_future! {
    /// Response future for [`MethodNotAllowed`](super::MethodNotAllowed).
    pub type MethodNotAllowedFuture<E> =
        std::future::Ready<Result<Response<BoxBody>, E>>;
}

pin_project! {
    /// The response future for [`Nested`](super::Nested).
    #[derive(Debug)]
    pub(crate) struct NestedFuture<S, B>
    where
        S: Service<Request<B>>,
    {
        #[pin]
        pub(super) inner: Oneshot<S, Request<B>>
    }
}

impl<S, B> Future for NestedFuture<S, B>
where
    S: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>,
    B: Send + Sync + 'static,
{
    type Output = Result<Response<BoxBody>, Infallible>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

opaque_future! {
    /// Response future from [`MakeRouteService`] services.
    pub type MakeRouteServiceFuture<S> =
        std::future::Ready<Result<S, Infallible>>;
}
