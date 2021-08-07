//! Future types.

use crate::{body::BoxBody, buffer::MpscBuffer};
use http::{Request, Response};
use pin_project_lite::pin_project;
use std::{
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{
    util::{BoxService, Oneshot},
    BoxError, Service,
};

opaque_future! {
    /// Response future for [`EmptyRouter`](super::EmptyRouter).
    pub type EmptyRouterFuture<E> =
        futures_util::future::Ready<Result<Response<BoxBody>, E>>;
}

pin_project! {
    /// The response future for [`BoxRoute`](super::BoxRoute).
    pub struct BoxRouteFuture<B, E>
    where
        E: Into<BoxError>,
    {
        #[pin]
        pub(super) inner: Oneshot<
            MpscBuffer<
                BoxService<Request<B>, Response<BoxBody>, E >,
                Request<B>
            >,
            Request<B>,
        >,
    }
}

impl<B, E> Future for BoxRouteFuture<B, E>
where
    E: Into<BoxError>,
{
    type Output = Result<Response<BoxBody>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

impl<B, E> fmt::Debug for BoxRouteFuture<B, E>
where
    E: Into<BoxError>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxRouteFuture").finish()
    }
}

pin_project! {
    /// The response future for [`Route`](super::Route).
    #[derive(Debug)]
    pub struct RouteFuture<S, F, B>
    where
        S: Service<Request<B>>,
        F: Service<Request<B>>
    {
        #[pin]
        inner: RouteFutureInner<S, F, B>,
    }
}

impl<S, F, B> RouteFuture<S, F, B>
where
    S: Service<Request<B>>,
    F: Service<Request<B>>,
{
    pub(crate) fn a(a: Oneshot<S, Request<B>>) -> Self {
        RouteFuture {
            inner: RouteFutureInner::A { a },
        }
    }

    pub(crate) fn b(b: Oneshot<F, Request<B>>) -> Self {
        RouteFuture {
            inner: RouteFutureInner::B { b },
        }
    }
}

pin_project! {
    #[project = RouteFutureInnerProj]
    #[derive(Debug)]
    enum RouteFutureInner<S, F, B>
    where
        S: Service<Request<B>>,
        F: Service<Request<B>>,
    {
        A { #[pin] a: Oneshot<S, Request<B>> },
        B { #[pin] b: Oneshot<F, Request<B>> },
    }
}

impl<S, F, B> Future for RouteFuture<S, F, B>
where
    S: Service<Request<B>, Response = Response<BoxBody>>,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error>,
{
    type Output = Result<Response<BoxBody>, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().inner.project() {
            RouteFutureInnerProj::A { a } => a.poll(cx),
            RouteFutureInnerProj::B { b } => b.poll(cx),
        }
    }
}

pin_project! {
    /// The response future for [`Nested`](super::Nested).
    #[derive(Debug)]
    pub struct NestedFuture<S, F, B>
    where
        S: Service<Request<B>>,
        F: Service<Request<B>>
    {
        #[pin]
        pub(super) inner: RouteFuture<S, F, B>,
    }
}

impl<S, F, B> Future for NestedFuture<S, F, B>
where
    S: Service<Request<B>, Response = Response<BoxBody>>,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = S::Error>,
{
    type Output = Result<Response<BoxBody>, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}
