//! [`Or`] used to combine two services into one.

use super::{FromEmptyRouter, OrDepth};
use crate::body::BoxBody;
use futures_util::ready;
use http::{Request, Response};
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{util::Oneshot, Service, ServiceExt};

/// [`tower::Service`] that is the combination of two routers.
///
/// See [`Router::or`] for more details.
///
/// [`Router::or`]: super::Router::or
#[derive(Debug, Clone, Copy)]
pub struct Or<A, B> {
    pub(super) first: A,
    pub(super) second: B,
}

#[allow(warnings)]
impl<A, B, ReqBody> Service<Request<ReqBody>> for Or<A, B>
where
    A: Service<Request<ReqBody>, Response = Response<BoxBody>> + Clone,
    B: Service<Request<ReqBody>, Response = Response<BoxBody>, Error = A::Error> + Clone,
    ReqBody: Send + Sync + 'static,
    A: Send + 'static,
    B: Send + 'static,
    A::Future: Send + 'static,
    B::Future: Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = A::Error;
    type Future = ResponseFuture<A, B, ReqBody>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        let original_uri = req.uri().clone();

        if let Some(count) = req.extensions_mut().get_mut::<OrDepth>() {
            count.increment();
        } else {
            req.extensions_mut().insert(OrDepth::new());
        }

        ResponseFuture {
            state: State::FirstFuture {
                f: self.first.clone().oneshot(req),
            },
            second: Some(self.second.clone()),
            original_uri: Some(original_uri),
        }
    }
}

pin_project! {
    /// Response future for [`Or`].
    pub struct ResponseFuture<A, B, ReqBody>
    where
        A: Service<Request<ReqBody>>,
        B: Service<Request<ReqBody>>,
    {
        #[pin]
        state: State<A, B, ReqBody>,
        second: Option<B>,
        // Some services, namely `Nested`, mutates the request URI so we must
        // restore it to its original state before calling `second`
        original_uri: Option<http::Uri>,
    }
}

pin_project! {
    #[project = StateProj]
    enum State<A, B, ReqBody>
    where
        A: Service<Request<ReqBody>>,
        B: Service<Request<ReqBody>>,
    {
        FirstFuture { #[pin] f: Oneshot<A, Request<ReqBody>> },
        SecondFuture {
            #[pin]
            f: Oneshot<B, Request<ReqBody>>,
        }
    }
}

impl<A, B, ReqBody> Future for ResponseFuture<A, B, ReqBody>
where
    A: Service<Request<ReqBody>, Response = Response<BoxBody>>,
    B: Service<Request<ReqBody>, Response = Response<BoxBody>, Error = A::Error>,
    ReqBody: Send + Sync + 'static,
{
    type Output = Result<Response<BoxBody>, A::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();

            let new_state = match this.state.as_mut().project() {
                StateProj::FirstFuture { f } => {
                    let mut response = ready!(f.poll(cx)?);

                    let mut req = if let Some(ext) = response
                        .extensions_mut()
                        .remove::<FromEmptyRouter<ReqBody>>()
                    {
                        ext.request
                    } else {
                        return Poll::Ready(Ok(response));
                    };

                    *req.uri_mut() = this.original_uri.take().unwrap();

                    let mut leaving_outermost_or = false;
                    if let Some(depth) = req.extensions_mut().get_mut::<OrDepth>() {
                        if depth == 1 {
                            leaving_outermost_or = true;
                        } else {
                            depth.decrement();
                        }
                    }
                    if leaving_outermost_or {
                        req.extensions_mut().remove::<OrDepth>();
                    }

                    let second = this.second.take().expect("future polled after completion");

                    State::SecondFuture {
                        f: second.oneshot(req),
                    }
                }
                StateProj::SecondFuture { f } => return f.poll(cx),
            };

            this.state.set(new_state);
        }
    }
}
