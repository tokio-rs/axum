use std::{future::Future, pin::Pin, task::Poll};

use axum::body::BoxBody;
use futures::ready;
use hyper::{Body, Request, Response};
use pin_project::pin_project;
use tower::Service;

#[derive(Clone)]
pub struct HybridService<Web, Grpc> {
    web: Web,
    grpc: Grpc,
}

impl<Web, Grpc> HybridService<Web, Grpc> {
    pub fn new(web: Web, grpc: Grpc) -> Self {
        HybridService { web, grpc }
    }
}

impl<Web, Grpc, Error> Service<Request<Body>> for HybridService<Web, Grpc>
where
    Web: Service<Request<Body>, Response = Response<BoxBody>, Error = Error>,
    Grpc: Service<Request<Body>, Response = Response<BoxBody>, Error = Error>,
{
    type Error = Error;
    type Future = HybridFuture<Web::Future, Grpc::Future>;
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

        if let Some(val) = hv && b"application/grpc".eq(val)  {
						return HybridFuture::Grpc(self.grpc.call(req));
				}

        HybridFuture::Web(self.web.call(req))
    }
}

#[pin_project(project = HybridFutureProj)]
pub enum HybridFuture<WebFuture, GrpcFuture> {
    Web(#[pin] WebFuture),
    Grpc(#[pin] GrpcFuture),
}

impl<WebFuture, GrpcFuture, Output> Future for HybridFuture<WebFuture, GrpcFuture>
where
    WebFuture: Future<Output = Output>,
    GrpcFuture: Future<Output = Output>,
{
    type Output = Output;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context) -> Poll<Self::Output> {
        match self.project() {
            HybridFutureProj::Web(a) => a.poll(cx),
            HybridFutureProj::Grpc(b) => b.poll(cx),
        }
    }
}
