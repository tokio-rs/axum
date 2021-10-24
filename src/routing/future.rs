//! Future types.

use crate::{
    body::BoxBody,
    routing::{FromEmptyRouter, UriStack},
};
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
    /// Response future for [`EmptyRouter`](super::EmptyRouter).
    pub type EmptyRouterFuture<E> =
        std::future::Ready<Result<Response<BoxBody>, E>>;
}

opaque_future! {
    /// Response future for [`Routes`](super::Routes).
    pub type RoutesFuture =
        futures_util::future::BoxFuture<'static, Result<Response<BoxBody>, Infallible>>;
}

pin_project! {
    /// The response future for [`Route`](super::Route).
    #[derive(Debug)]
    pub(crate) struct RouteFuture<S, F, B>
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
    pub(crate) struct NestedFuture<S, F, B>
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
