use std::task::Poll;

use axum::body::BoxBody;
use futures::{
    future::{BoxFuture, Either},
    ready,
};
use hyper::{Body, Request, Response};
use tower::{make::Shared, Service};

#[derive(Clone)]
pub struct MultiplexService<A, B> {
    pub web: A,
    pub grpc: B,
}

impl<A, B> MultiplexService<A, B> {
    pub fn make_shared(self) -> tower::make::Shared<Self> {
        Shared::new(self)
    }
}

impl<A, B, Error> Service<Request<Body>> for MultiplexService<A, B>
where
    A: Service<Request<Body>, Response = Response<BoxBody>, Error = Error>,
    A::Future: Send + 'static,
    A::Error: 'static,
    B: Service<Request<Body>, Response = Response<BoxBody>, Error = Error>,
    B::Future: Send + 'static,
    B::Error: 'static,
{
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    type Response = Response<BoxBody>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(if let Err(err) = ready!(self.web.poll_ready(cx)) {
            Err(err)
        } else {
            ready!(self.web.poll_ready(cx))
        })
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let hv = req.headers().get("content-type").map(|x| x.as_bytes());

        let fut = if hv
            .filter(|value| value.starts_with(b"application/grpc"))
            .is_some()
        {
            Either::Left(self.grpc.call(req))
        } else {
            Either::Right(self.web.call(req))
        };

        Box::pin(fut)
    }
}
