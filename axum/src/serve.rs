//! Serve services.

use std::{
    convert::Infallible,
    future::Future,
    io,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use axum_core::{body::Body, extract::Request, response::Response};
use futures_util::future::poll_fn;
use hyper::body::Incoming;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use pin_project_lite::pin_project;
use tokio::net::{TcpListener, TcpStream};
use tower::util::{Oneshot, ServiceExt};
use tower_service::Service;

/// Serve the service with the supplied listener.
///
/// This method of running a service is intentionally simple and doesn't support any configuration.
/// Use hyper or hyper-util if you need configuration.
///
/// It supports both HTTP/1 as well as HTTP/2.
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
#[cfg(all(feature = "tokio", feature = "http1"))]
pub async fn serve<M, S>(tcp_listener: TcpListener, mut make_service: M) -> io::Result<()>
where
    M: for<'a> Service<IncomingStream<'a>, Error = Infallible, Response = S>,
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    loop {
        let (tcp_stream, remote_addr) = tcp_listener.accept().await?;
        let tcp_stream = TokioIo::new(tcp_stream);

        poll_fn(|cx| make_service.poll_ready(cx))
            .await
            .unwrap_or_else(|err| match err {});

        let tower_service = make_service
            .call(IncomingStream {
                tcp_stream: &tcp_stream,
                remote_addr,
            })
            .await
            .unwrap_or_else(|err| match err {});

        let hyper_service = TowerToHyperService {
            service: tower_service,
        };

        tokio::task::spawn(async move {
            match Builder::new(TokioExecutor::new())
                // upgrades needed for websockets
                .serve_connection_with_upgrades(tcp_stream.into_inner(), hyper_service)
                .await
            {
                Ok(()) => {}
                Err(_err) => {
                    // This error only appears when the client doesn't send a request and
                    // terminate the connection.
                    //
                    // If client sends one request then terminate connection whenever, it doesn't
                    // appear.
                }
            }
        });
    }
}

#[derive(Debug, Copy, Clone)]
struct TowerToHyperService<S> {
    service: S,
}

impl<S> hyper::service::Service<Request<Incoming>> for TowerToHyperService<S>
where
    S: tower_service::Service<Request> + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = TowerToHyperServiceFuture<S, Request>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let req = req.map(Body::new);
        TowerToHyperServiceFuture {
            future: self.service.clone().oneshot(req),
        }
    }
}

pin_project! {
    struct TowerToHyperServiceFuture<S, R>
    where
        S: tower_service::Service<R>,
    {
        #[pin]
        future: Oneshot<S, R>,
    }
}

impl<S, R> Future for TowerToHyperServiceFuture<S, R>
where
    S: tower_service::Service<R>,
{
    type Output = Result<S::Response, S::Error>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().future.poll(cx)
    }
}

/// An incoming stream.
///
/// Used with [`serve`] and [`IntoMakeServiceWithConnectInfo`].
///
/// [`IntoMakeServiceWithConnectInfo`]: crate::extract::connect_info::IntoMakeServiceWithConnectInfo
#[derive(Debug)]
pub struct IncomingStream<'a> {
    tcp_stream: &'a TokioIo<TcpStream>,
    remote_addr: SocketAddr,
}

impl IncomingStream<'_> {
    /// Returns the local address that this stream is bound to.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.tcp_stream.inner().local_addr()
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
