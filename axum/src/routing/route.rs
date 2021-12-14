use crate::{
    body::{boxed, Body, Empty},
    response::Response,
};
use http::Request;
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{
    util::{BoxCloneService, Oneshot},
    ServiceExt,
};
use tower_service::Service;

/// How routes are stored inside a [`Router`](super::Router).
///
/// You normally shouldn't need to care about this type. It's used in
/// [`Router::layer`](super::Router::layer).
pub struct Route<B = Body, E = Infallible>(pub(crate) BoxCloneService<Request<B>, Response, E>);

impl<B, E> Route<B, E> {
    pub(super) fn new<T>(svc: T) -> Self
    where
        T: Service<Request<B>, Response = Response, Error = E> + Clone + Send + 'static,
        T::Future: Send + 'static,
    {
        Self(BoxCloneService::new(svc))
    }
}

impl<ReqBody, E> Clone for Route<ReqBody, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<ReqBody, E> fmt::Debug for Route<ReqBody, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Route").finish()
    }
}

impl<B, E> Service<Request<B>> for Route<B, E> {
    type Response = Response;
    type Error = E;
    type Future = RouteFuture<B, E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        RouteFuture::new(self.0.clone().oneshot(req))
    }
}

pin_project! {
    /// Response future for [`Route`].
    pub struct RouteFuture<B, E> {
        #[pin]
        future: Oneshot<
            BoxCloneService<Request<B>, Response, E>,
            Request<B>,
        >,
        strip_body: bool,
    }
}

impl<B, E> RouteFuture<B, E> {
    pub(crate) fn new(
        future: Oneshot<BoxCloneService<Request<B>, Response, E>, Request<B>>,
    ) -> Self {
        RouteFuture {
            future,
            strip_body: false,
        }
    }

    pub(crate) fn strip_body(mut self, strip_body: bool) -> Self {
        self.strip_body = strip_body;
        self
    }
}

impl<B, E> Future for RouteFuture<B, E> {
    type Output = Result<Response, E>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let strip_body = self.strip_body;

        match self.project().future.poll(cx) {
            Poll::Ready(Ok(res)) => {
                if strip_body {
                    Poll::Ready(Ok(res.map(|_| boxed(Empty::new()))))
                } else {
                    Poll::Ready(Ok(res))
                }
            }
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traits() {
        use crate::test_helpers::*;
        assert_send::<Route<()>>();
    }
}
