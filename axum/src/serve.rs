use std::{
    convert::Infallible,
    io,
    net::SocketAddr,
    task::{Context, Poll},
};

use axum_core::{body::Body, extract::Request, response::Response};
use futures_util::{future::poll_fn, FutureExt};
use hyper1::server::conn::http1;
use tokio::net::{TcpListener, TcpStream};
use tower_hyper_http_body_compat::{HttpBody04ToHttpBody1, HttpBody1ToHttpBody04};
use tower_service::Service;

/// TODO(david): docs
pub async fn serve<M, S>(tcp_listener: TcpListener, mut make_service: M) -> io::Result<()>
where
    M: for<'a> Service<&'a (TcpStream, SocketAddr), Error = Infallible, Response = S>,
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    loop {
        let target = tcp_listener.accept().await?;

        poll_fn(|cx| make_service.poll_ready(cx))
            .await
            .unwrap_or_else(|err| match err {});

        let mut service = make_service
            .call(&target)
            .await
            .unwrap_or_else(|err| match err {});

        let (tcp_stream, _) = target;

        let service = hyper1::service::service_fn(move |req: Request<hyper1::body::Incoming>| {
            let req = req.map(|body| {
                // wont need this when axum uses http-body 1.0
                let http_body_04 = HttpBody1ToHttpBody04::new(body);
                Body::new(http_body_04)
            });

            // doing this saves cloning the service just to await the service being ready
            //
            // services like `Router` are always ready, so assume the service
            // we're running here is also always ready...
            let waker = futures_util::task::noop_waker();
            let mut cx = Context::from_waker(&waker);
            match service.poll_ready(&mut cx) {
                Poll::Ready(Ok(())) => {}
                Poll::Ready(Err(err)) => match err {},
                // ...otherwise load shed
                Poll::Pending => {
                    let mut res = Response::new(HttpBody04ToHttpBody1::new(Body::empty()));
                    *res.status_mut() = http::StatusCode::SERVICE_UNAVAILABLE;
                    return std::future::ready(Ok(res)).left_future();
                }
            }

            let future = service.call(req);

            async move {
                let response = future
                    .await
                    .unwrap_or_else(|err| match err {})
                    // wont need this when axum uses http-body 1.0
                    .map(HttpBody04ToHttpBody1::new);

                Ok::<_, Infallible>(response)
            }
            .right_future()
        });

        tokio::task::spawn(async move {
            match http1::Builder::new()
                .serve_connection(tcp_stream, service)
                .await
            {
                Ok(()) => {}
                Err(_err) => {
                    // TODO(david): how to handle this error?
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{handler::HandlerWithoutStateExt, routing::get, Router};

    #[allow(dead_code, unused_must_use)]
    async fn if_it_compiles_it_works() {
        let router: Router = Router::new();

        let addr = "0.0.0.0:0";

        // router
        serve(TcpListener::bind(addr).await.unwrap(), router.clone());
        serve(
            TcpListener::bind(addr).await.unwrap(),
            router.clone().into_make_service(),
        );
        serve(
            TcpListener::bind(addr).await.unwrap(),
            router.into_make_service_with_connect_info::<SocketAddr>(),
        );

        // method router
        serve(
            TcpListener::bind(addr).await.unwrap(),
            get(handler).into_make_service(),
        );
        serve(
            TcpListener::bind(addr).await.unwrap(),
            get(handler).into_make_service_with_connect_info::<SocketAddr>(),
        );

        // handler
        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_make_service(),
        );
        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_make_service_with_connect_info::<SocketAddr>(),
        );
    }

    async fn handler() {}
}
