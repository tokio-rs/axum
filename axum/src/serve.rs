//! Serve services.

use std::{
    convert::Infallible,
    fmt::Debug,
    future::{poll_fn, Future, IntoFuture},
    io,
    marker::PhantomData,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use axum_core::{body::Body, extract::Request, response::Response};
use futures_util::{pin_mut, FutureExt};
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
#[cfg(any(feature = "http1", feature = "http2"))]
use hyper_util::{server::conn::auto::Builder, service::TowerToHyperService};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::watch,
};
use tower::ServiceExt as _;
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
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
pub fn serve<M, S>(tcp_listener: TcpListener, make_service: M) -> Serve<M, S>
where
    M: for<'a> Service<IncomingStream<'a>, Error = Infallible, Response = S>,
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    Serve {
        tcp_listener,
        make_service,
        tcp_nodelay: None,
        _marker: PhantomData,
    }
}

/// Future returned by [`serve`].
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
#[must_use = "futures must be awaited or polled"]
pub struct Serve<M, S> {
    tcp_listener: TcpListener,
    make_service: M,
    tcp_nodelay: Option<bool>,
    _marker: PhantomData<S>,
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<M, S> Serve<M, S> {
    /// Prepares a server to handle graceful shutdown when the provided future completes.
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{Router, routing::get};
    ///
    /// # async {
    /// let router = Router::new().route("/", get(|| async { "Hello, World!" }));
    ///
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, router)
    ///     .with_graceful_shutdown(shutdown_signal())
    ///     .await
    ///     .unwrap();
    /// # };
    ///
    /// async fn shutdown_signal() {
    ///     // ...
    /// }
    /// ```
    pub fn with_graceful_shutdown<F>(self, signal: F) -> WithGracefulShutdown<M, S, F>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        WithGracefulShutdown {
            tcp_listener: self.tcp_listener,
            make_service: self.make_service,
            signal,
            tcp_nodelay: self.tcp_nodelay,
            _marker: PhantomData,
        }
    }

    /// Instructs the server to set the value of the `TCP_NODELAY` option on every accepted connection.
    ///
    /// See also [`TcpStream::set_nodelay`].
    ///
    /// # Example
    /// ```
    /// use axum::{Router, routing::get};
    ///
    /// # async {
    /// let router = Router::new().route("/", get(|| async { "Hello, World!" }));
    ///
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, router)
    ///     .tcp_nodelay(true)
    ///     .await
    ///     .unwrap();
    /// # };
    /// ```
    pub fn tcp_nodelay(self, nodelay: bool) -> Self {
        Self {
            tcp_nodelay: Some(nodelay),
            ..self
        }
    }

    /// Returns the local address this server is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp_listener.local_addr()
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<M, S> Debug for Serve<M, S>
where
    M: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            tcp_listener,
            make_service,
            tcp_nodelay,
            _marker: _,
        } = self;

        f.debug_struct("Serve")
            .field("tcp_listener", tcp_listener)
            .field("make_service", make_service)
            .field("tcp_nodelay", tcp_nodelay)
            .finish()
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<M, S> IntoFuture for Serve<M, S>
where
    M: for<'a> Service<IncomingStream<'a>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a>>>::Future: Send,
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    type Output = io::Result<()>;
    type IntoFuture = private::ServeFuture;

    fn into_future(self) -> Self::IntoFuture {
        private::ServeFuture(Box::pin(async move {
            let Self {
                tcp_listener,
                mut make_service,
                tcp_nodelay,
                _marker: _,
            } = self;

            loop {
                let (tcp_stream, remote_addr) = match tcp_accept(&tcp_listener).await {
                    Some(conn) => conn,
                    None => continue,
                };

                if let Some(nodelay) = tcp_nodelay {
                    if let Err(err) = tcp_stream.set_nodelay(nodelay) {
                        trace!("failed to set TCP_NODELAY on incoming connection: {err:#}");
                    }
                }

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
                    .unwrap_or_else(|err| match err {})
                    .map_request(|req: Request<Incoming>| req.map(Body::new));

                let hyper_service = TowerToHyperService::new(tower_service);

                tokio::spawn(async move {
                    match Builder::new(TokioExecutor::new())
                        // upgrades needed for websockets
                        .serve_connection_with_upgrades(tcp_stream, hyper_service)
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
        }))
    }
}

/// Serve future with graceful shutdown enabled.
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
#[must_use = "futures must be awaited or polled"]
pub struct WithGracefulShutdown<M, S, F> {
    tcp_listener: TcpListener,
    make_service: M,
    signal: F,
    tcp_nodelay: Option<bool>,
    _marker: PhantomData<S>,
}

impl<M, S, F> WithGracefulShutdown<M, S, F> {
    /// Instructs the server to set the value of the `TCP_NODELAY` option on every accepted connection.
    ///
    /// See also [`TcpStream::set_nodelay`].
    ///
    /// # Example
    /// ```
    /// use axum::{Router, routing::get};
    ///
    /// # async {
    /// let router = Router::new().route("/", get(|| async { "Hello, World!" }));
    ///
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, router)
    ///     .with_graceful_shutdown(shutdown_signal())
    ///     .tcp_nodelay(true)
    ///     .await
    ///     .unwrap();
    /// # };
    ///
    /// async fn shutdown_signal() {
    ///     // ...
    /// }
    /// ```
    pub fn tcp_nodelay(self, nodelay: bool) -> Self {
        Self {
            tcp_nodelay: Some(nodelay),
            ..self
        }
    }

    /// Returns the local address this server is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp_listener.local_addr()
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<M, S, F> Debug for WithGracefulShutdown<M, S, F>
where
    M: Debug,
    S: Debug,
    F: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            tcp_listener,
            make_service,
            signal,
            tcp_nodelay,
            _marker: _,
        } = self;

        f.debug_struct("WithGracefulShutdown")
            .field("tcp_listener", tcp_listener)
            .field("make_service", make_service)
            .field("signal", signal)
            .field("tcp_nodelay", tcp_nodelay)
            .finish()
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<M, S, F> IntoFuture for WithGracefulShutdown<M, S, F>
where
    M: for<'a> Service<IncomingStream<'a>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a>>>::Future: Send,
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    F: Future<Output = ()> + Send + 'static,
{
    type Output = io::Result<()>;
    type IntoFuture = private::ServeFuture;

    fn into_future(self) -> Self::IntoFuture {
        let Self {
            tcp_listener,
            mut make_service,
            signal,
            tcp_nodelay,
            _marker: _,
        } = self;

        let (signal_tx, signal_rx) = watch::channel(());
        let signal_tx = Arc::new(signal_tx);
        tokio::spawn(async move {
            signal.await;
            trace!("received graceful shutdown signal. Telling tasks to shutdown");
            drop(signal_rx);
        });

        let (close_tx, close_rx) = watch::channel(());

        private::ServeFuture(Box::pin(async move {
            loop {
                let (tcp_stream, remote_addr) = tokio::select! {
                    conn = tcp_accept(&tcp_listener) => {
                        match conn {
                            Some(conn) => conn,
                            None => continue,
                        }
                    }
                    _ = signal_tx.closed() => {
                        trace!("signal received, not accepting new connections");
                        break;
                    }
                };

                if let Some(nodelay) = tcp_nodelay {
                    if let Err(err) = tcp_stream.set_nodelay(nodelay) {
                        trace!("failed to set TCP_NODELAY on incoming connection: {err:#}");
                    }
                }

                let tcp_stream = TokioIo::new(tcp_stream);

                trace!("connection {remote_addr} accepted");

                poll_fn(|cx| make_service.poll_ready(cx))
                    .await
                    .unwrap_or_else(|err| match err {});

                let tower_service = make_service
                    .call(IncomingStream {
                        tcp_stream: &tcp_stream,
                        remote_addr,
                    })
                    .await
                    .unwrap_or_else(|err| match err {})
                    .map_request(|req: Request<Incoming>| req.map(Body::new));

                let hyper_service = TowerToHyperService::new(tower_service);

                let signal_tx = Arc::clone(&signal_tx);

                let close_rx = close_rx.clone();

                tokio::spawn(async move {
                    let builder = Builder::new(TokioExecutor::new());
                    let conn = builder.serve_connection_with_upgrades(tcp_stream, hyper_service);
                    pin_mut!(conn);

                    let signal_closed = signal_tx.closed().fuse();
                    pin_mut!(signal_closed);

                    loop {
                        tokio::select! {
                            result = conn.as_mut() => {
                                if let Err(_err) = result {
                                    trace!("failed to serve connection: {_err:#}");
                                }
                                break;
                            }
                            _ = &mut signal_closed => {
                                trace!("signal received in task, starting graceful shutdown");
                                conn.as_mut().graceful_shutdown();
                            }
                        }
                    }

                    trace!("connection {remote_addr} closed");

                    drop(close_rx);
                });
            }

            drop(close_rx);
            drop(tcp_listener);

            trace!(
                "waiting for {} task(s) to finish",
                close_tx.receiver_count()
            );
            close_tx.closed().await;

            Ok(())
        }))
    }
}

fn is_connection_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}

async fn tcp_accept(listener: &TcpListener) -> Option<(TcpStream, SocketAddr)> {
    match listener.accept().await {
        Ok(conn) => Some(conn),
        Err(e) => {
            if is_connection_error(&e) {
                return None;
            }

            // [From `hyper::Server` in 0.14](https://github.com/hyperium/hyper/blob/v0.14.27/src/server/tcp.rs#L186)
            //
            // > A possible scenario is that the process has hit the max open files
            // > allowed, and so trying to accept a new connection will fail with
            // > `EMFILE`. In some cases, it's preferable to just wait for some time, if
            // > the application will likely close some files (or connections), and try
            // > to accept the connection again. If this option is `true`, the error
            // > will be logged at the `error` level, since it is still a big deal,
            // > and then the listener will sleep for 1 second.
            //
            // hyper allowed customizing this but axum does not.
            error!("accept error: {e}");
            tokio::time::sleep(Duration::from_secs(1)).await;
            None
        }
    }
}

mod private {
    use std::{
        future::Future,
        io,
        pin::Pin,
        task::{Context, Poll},
    };

    pub struct ServeFuture(pub(super) futures_util::future::BoxFuture<'static, io::Result<()>>);

    impl Future for ServeFuture {
        type Output = io::Result<()>;

        #[inline]
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.0.as_mut().poll(cx)
        }
    }

    impl std::fmt::Debug for ServeFuture {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("ServeFuture").finish_non_exhaustive()
        }
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
    use std::{
        future::pending,
        net::{IpAddr, Ipv4Addr},
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

        // nodelay
        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_service(),
        )
        .tcp_nodelay(true);

        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_service(),
        )
        .with_graceful_shutdown(async { /*...*/ })
        .tcp_nodelay(true);
    }

    async fn handler() {}

    #[crate::test]
    async fn test_serve_local_addr() {
        let router: Router = Router::new();
        let addr = "0.0.0.0:0";

        let server = serve(TcpListener::bind(addr).await.unwrap(), router.clone());
        let address = server.local_addr().unwrap();

        assert_eq!(address.ip(), IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
        assert_ne!(address.port(), 0);
    }

    #[crate::test]
    async fn test_with_graceful_shutdown_local_addr() {
        let router: Router = Router::new();
        let addr = "0.0.0.0:0";

        let server = serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .with_graceful_shutdown(pending());
        let address = server.local_addr().unwrap();

        assert_eq!(address.ip(), IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
        assert_ne!(address.port(), 0);
    }
}
