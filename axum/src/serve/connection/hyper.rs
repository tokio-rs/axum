use std::{
    convert::Infallible,
    error::Error as StdError,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum_core::{body::Body, extract::Request, response::Response};
use http_body::Body as HttpBody;
use hyper::{
    body::Incoming,
    rt::{Read as HyperRead, Write as HyperWrite},
    service::HttpService as HyperHttpService,
    service::Service as HyperService,
};
#[cfg(feature = "http1")]
use hyper_util::rt::TokioTimer;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::{Builder, HttpServerConnExec, UpgradeableConnection},
    service::TowerToHyperService,
};
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::sync::{CancellationToken, WaitForCancellationFutureOwned};
use tower::{Service, ServiceExt};

use super::{Connection, ConnectionBuilder};

pin_project! {
    /// An implementation of [`Connection`] when serving with hyper.
    pub struct HyperConnection<'a, I, S: HyperHttpService<Incoming>, E> {
        #[pin]
        inner: UpgradeableConnection<'a, I, S, E>,
        #[pin]
        shutdown: Option<WaitForCancellationFutureOwned>,
    }
}

impl<I, S, E, B> Connection for HyperConnection<'_, I, S, E>
where
    S: HyperService<Request<Incoming>, Response = Response<B>> + Send,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    S::Future: Send + 'static,
    I: HyperRead + HyperWrite + Unpin + Send + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: HttpServerConnExec<S::Future, B> + Send + Sync,
{
    fn poll_connection(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Box<dyn StdError + Send + Sync>>> {
        let mut this = self.project();
        if let Some(shutdown) = this.shutdown.as_mut().as_pin_mut() {
            if shutdown.poll(cx).is_ready() {
                trace!("signal received in connection, starting graceful shutdown");
                this.inner.as_mut().graceful_shutdown();
                this.shutdown.set(None);
            }
        }
        this.inner.poll(cx)
    }
}

/// An implementation of [`ConnectionBuilder`] when serving with hyper.
#[derive(Debug, Clone)]
pub struct Hyper {
    builder: Builder<TokioExecutor>,
    shutdown: CancellationToken,
}

impl Hyper {
    /// Create a new [`ConnectionBuilder`] implementation from a
    /// [`hyper_util::server::conn::auto::Builder`]. This builder may be set up in any way that the
    /// user may need.
    ///
    /// # Example
    ///
    /// ```rust
    /// # async {
    /// # use axum::Router;
    /// # use axum::serve::{Hyper, serve};
    /// # use hyper_util::server::conn::auto::Builder;
    /// # use hyper_util::rt::TokioExecutor;
    /// let mut builder = Builder::new(TokioExecutor::new()).http2_only();
    /// builder.http2().enable_connect_protocol();
    /// let connection_builder = Hyper::new(builder);
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// serve(listener, Router::new()).with_connection_builder(connection_builder).await.unwrap();
    /// # };
    /// ```
    #[must_use]
    pub fn new(builder: Builder<TokioExecutor>) -> Self {
        Self {
            builder,
            shutdown: CancellationToken::new(),
        }
    }
}

impl Default for Hyper {
    fn default() -> Self {
        #[allow(unused_mut)]
        let mut builder = Builder::new(TokioExecutor::new());

        // Enable Hyper's default HTTP/1 request header timeout.
        #[cfg(feature = "http1")]
        builder.http1().timer(TokioTimer::new());

        // CONNECT protocol needed for HTTP/2 websockets
        #[cfg(feature = "http2")]
        builder.http2().enable_connect_protocol();

        Self::new(builder)
    }
}

impl<Io, S, B> ConnectionBuilder<Io, S> for Hyper
where
    Io: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    S: Service<Request, Response = Response<B>, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    fn build_connection(&mut self, io: Io, service: S) -> impl Connection {
        fn map_body(req: Request<Incoming>) -> Request {
            req.map(Body::new)
        }

        let hyper_service = TowerToHyperService::new(
            service.map_request(map_body as fn(Request<Incoming>) -> Request),
        );

        let io = TokioIo::new(io);
        let hyper_connection = self
            .builder
            .serve_connection_with_upgrades(io, hyper_service);

        HyperConnection {
            inner: hyper_connection,
            shutdown: Some(self.shutdown.clone().cancelled_owned()),
        }
    }

    fn graceful_shutdown(&mut self) {
        self.shutdown.cancel();
    }
}
