//! Serve services.

use std::{convert::Infallible, io, net::SocketAddr};

use axum_core::{body::Body, extract::Request, response::Response};
use futures_util::{future::poll_fn, FutureExt};
use hyper1::server::conn::http1;
use tokio::net::{TcpListener, TcpStream};
use tower_hyper_http_body_compat::{HttpBody04ToHttpBody1, HttpBody1ToHttpBody04};
use tower_service::Service;

/// Serve the service with the supplied listener.
///
/// This method of running a service is intentionally simple and doesn't support any configuration.
/// Use hyper or hyper-util if you need configuration.
///
/// It only supports HTTP/1.
///
/// # Examples
///
/// Serving a [`Router`]:
///
/// ```
/// use axum::{Router, routing::get};
///
/// # async {
/// let router = Router::new().route("/", get(|| async { "Hello, World!" }));
///
/// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
/// axum::serve(listener, router).await.unwrap();
/// # };
/// ```
///
/// See also [`Router::into_make_service_with_connect_info`].
///
/// Serving a [`MethodRouter`]:
///
/// ```
/// use axum::routing::get;
///
/// # async {
/// let router = get(|| async { "Hello, World!" });
///
/// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
/// axum::serve(listener, router).await.unwrap();
/// # };
/// ```
///
/// See also [`MethodRouter::into_make_service_with_connect_info`].
///
/// Serving a [`Handler`]:
///
/// ```
/// use axum::handler::HandlerWithoutStateExt;
///
/// # async {
/// async fn handler() -> &'static str {
///     "Hello, World!"
/// }
///
/// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
/// axum::serve(listener, handler.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// See also [`HandlerWithoutStateExt::into_make_service_with_connect_info`] and
/// [`HandlerService::into_make_service_with_connect_info`].
///
/// [`Router`]: crate::Router
/// [`Router::into_make_service_with_connect_info`]: crate::Router::into_make_service_with_connect_info
/// [`MethodRouter`]: crate::routing::MethodRouter
/// [`MethodRouter::into_make_service_with_connect_info`]: crate::routing::MethodRouter::into_make_service_with_connect_info
/// [`Handler`]: crate::handler::Handler
/// [`HandlerWithoutStateExt::into_make_service_with_connect_info`]: crate::handler::HandlerWithoutStateExt::into_make_service_with_connect_info
/// [`HandlerService::into_make_service_with_connect_info`]: crate::handler::HandlerService::into_make_service_with_connect_info
#[cfg(feature = "tokio")]
pub async fn serve<M, S>(tcp_listener: TcpListener, mut make_service: M) -> io::Result<()>
where
    M: for<'a> Service<IncomingStream<'a>, Error = Infallible, Response = S>,
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    loop {
        let (tcp_stream, remote_addr) = tcp_listener.accept().await?;

        poll_fn(|cx| make_service.poll_ready(cx))
            .await
            .unwrap_or_else(|err| match err {});

        let mut service = make_service
            .call(IncomingStream {
                tcp_stream: &tcp_stream,
                remote_addr,
            })
            .await
            .unwrap_or_else(|err| match err {});

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
            match futures_util::future::poll_fn(|cx| service.poll_ready(cx)).now_or_never() {
                Some(Ok(())) => {}
                Some(Err(err)) => match err {},
                None => {
                    // ...otherwise load shed
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
                // for websockets
                .with_upgrades()
                .await
            {
                Ok(()) => {}
                Err(_err) => {
                    // This error only appears when  the client doesn't send a request and
                    // terminate the connection.
                    //
                    // If client sends one request then terminate connection whenever, it doesn't
                    // appear.
                }
            }
        });
    }
}

/// An incoming stream.
///
/// Used with [`serve`] and [`IntoMakeServiceWithConnectInfo`].
///
/// [`IntoMakeServiceWithConnectInfo`]: crate::extract::connect_info::IntoMakeServiceWithConnectInfo
#[derive(Debug)]
pub struct IncomingStream<'a> {
    tcp_stream: &'a TcpStream,
    remote_addr: SocketAddr,
}

impl IncomingStream<'_> {
    /// Returns the local address that this stream is bound to.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.tcp_stream.local_addr()
    }

    /// Returns the remote address that this stream is bound to.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        handler::{Handler, HandlerWithoutStateExt},
        routing::get,
        Router,
    };

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
        serve(TcpListener::bind(addr).await.unwrap(), get(handler));
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
            handler.into_service(),
        );
        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.with_state(()),
        );
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
