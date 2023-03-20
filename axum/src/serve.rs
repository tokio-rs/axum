//! TODO(david): docs

#![allow(unused_imports, missing_docs)]

use std::{
    convert::Infallible,
    future::Future,
    io,
    net::SocketAddr,
    task::{Context, Poll},
};

use axum_core::{
    body::Body,
    extract::Request,
    response::{IntoResponse, Response},
};
use futures_util::future::poll_fn;
use hyper1::server::conn::http1;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tower::ServiceExt;
use tower_hyper_http_body_compat::{
    HttpBody04ToHttpBody1, HttpBody1ToHttpBody04, TowerService03HttpServiceAsHyper1HttpService,
};
use tower_service::Service;

use crate::Router;

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

        let service = make_service
            .call(&target)
            .await
            .unwrap_or_else(|err| match err {});

        let (tcp_stream, _) = target;

        let service = hyper1::service::service_fn(move |req: Request<hyper1::body::Incoming>| {
            let mut service = service.clone();

            let req = req.map(|body| {
                // wont need this when axum uses http-body 1.0
                let http_body_04 = HttpBody1ToHttpBody04::new(body);
                Body::new(http_body_04)
            });

            async move {
                poll_fn(|cx| service.poll_ready(cx))
                    .await
                    .unwrap_or_else(|err| match err {});

                let response = service
                    .call(req)
                    .await
                    .unwrap_or_else(|err| match err {})
                    // wont need this when axum uses http-body 1.0
                    .map(HttpBody04ToHttpBody1::new);

                Ok::<_, Infallible>(response)
            }
        });

        tokio::task::spawn(async move {
            match http1::Builder::new()
                .serve_connection(tcp_stream, service)
                .await
            {
                Ok(()) => {}
                Err(err) => {
                    // TODO(david): how to handle this error?
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{handler::HandlerWithoutStateExt, routing::get};

    #[allow(dead_code, unused_must_use)]
    async fn if_it_compiles_it_works() {
        let router: Router = Router::new();

        let addr = "0.0.0.0:0";

        // router
        serve(
            TcpListener::bind(addr).await.unwrap(),
            router.clone(),
        );
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
