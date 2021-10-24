//! Future types.

use crate::{
    body::BoxBody,
    clone_box_service::CloneBoxService,
    routing::{FromEmptyRouter, UriStack},
};
use http::{Request, Response};
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::util::Oneshot;
use tower_service::Service;

pub use super::or::ResponseFuture as OrResponseFuture;

opaque_future! {
    /// Response future for [`EmptyRouter`](super::EmptyRouter).
    pub type EmptyRouterFuture<E> =
        std::future::Ready<Result<Response<BoxBody>, E>>;
}

pin_project! {
    /// The response future for [`BoxRoute`](super::BoxRoute).
    pub struct BoxRouteFuture<B> {
        #[pin]
        pub(super) inner: Oneshot<
            CloneBoxService<Request<B>, Response<BoxBody>, Infallible>,
            Request<B>,
        >,
    }
}

impl<B> Future for BoxRouteFuture<B> {
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

impl<B> fmt::Debug for BoxRouteFuture<B> {
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
        state: RouteFutureInner<S, F, B>,
    }
}

impl<S, F, B> RouteFuture<S, F, B>
where
    S: Service<Request<B>>,
    F: Service<Request<B>>,
{
    pub(crate) fn a(a: Oneshot<S, Request<B>>) -> Self {
        RouteFuture {
            state: RouteFutureInner::A { a },
        }
    }

    pub(crate) fn b(b: Oneshot<F, Request<B>>) -> Self {
        RouteFuture {
            state: RouteFutureInner::B { b },
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
        A {
            #[pin]
            a: Oneshot<S, Request<B>>,
        },
        B {
            #[pin]
            b: Oneshot<F, Request<B>>
        },
    }
}

impl<S, F, B> Future for RouteFuture<S, F, B>
where
    S: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>,
    B: Send + Sync + 'static,
{
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().state.project() {
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
    S: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>,
    F: Service<Request<B>, Response = Response<BoxBody>, Error = Infallible>,
    B: Send + Sync + 'static,
{
    type Output = Result<Response<BoxBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut res: Response<_> = futures_util::ready!(self.project().inner.poll(cx)?);

        // `nest` mutates the URI of the request so if it turns out no route matched
        // we need to reset the URI so the next routes see the original URI
        //
        // That requires using a stack since we can have arbitrarily nested routes
        if let Some(from_empty_router) = res.extensions_mut().get_mut::<FromEmptyRouter<B>>() {
            let uri = UriStack::pop(&mut from_empty_router.request);
            if let Some(uri) = uri {
                *from_empty_router.request.uri_mut() = uri;
            }
        }

        Poll::Ready(Ok(res))
    }
}

opaque_future! {
    /// Response future from [`MakeRouteService`] services.
    pub type MakeRouteServiceFuture<S> =
        std::future::Ready<Result<S, Infallible>>;
}
