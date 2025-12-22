//! Serve services.

use std::{
    convert::Infallible,
    error::Error as StdError,
    fmt::Debug,
    future::{poll_fn, Future, IntoFuture},
    io,
    marker::PhantomData,
    pin::pin,
};

use axum_core::{extract::Request, response::Response};
use http_body::Body as HttpBody;
use tokio::sync::watch;
use tower::ServiceExt as _;
use tower_service::Service;

mod connection;
mod listener;

pub use connection::{Connection, ConnectionBuilder, Hyper, HyperConnection};
pub use listener::{ConnLimiter, ConnLimiterIo, Listener, ListenerExt, TapIo};

/// Serve the service with the supplied listener.
///
/// This method of running a service is intentionally simple and doesn't support any configuration.
/// hyper's default configuration applies (including [timeouts]); use hyper or hyper-util if you
/// need configuration.
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
/// # Return Value
///
/// Although this future resolves to `io::Result<()>`, it will never actually complete or return an
/// error. Errors on the TCP socket will be handled by sleeping for a short while (currently, one
/// second).
///
/// [timeouts]: hyper::server::conn::http1::Builder::header_read_timeout
/// [`Router`]: crate::Router
/// [`Router::into_make_service_with_connect_info`]: crate::Router::into_make_service_with_connect_info
/// [`MethodRouter`]: crate::routing::MethodRouter
/// [`MethodRouter::into_make_service_with_connect_info`]: crate::routing::MethodRouter::into_make_service_with_connect_info
/// [`Handler`]: crate::handler::Handler
/// [`HandlerWithoutStateExt::into_make_service_with_connect_info`]: crate::handler::HandlerWithoutStateExt::into_make_service_with_connect_info
/// [`HandlerService::into_make_service_with_connect_info`]: crate::handler::HandlerService::into_make_service_with_connect_info
#[cfg(feature = "tokio")]
pub fn serve<L, M, S, B>(listener: L, make_service: M) -> Serve<L, Hyper, M, S, B>
where
    L: Listener,
    M: for<'a> Service<IncomingStream<'a, L>, Error = Infallible, Response = S>,
    S: Service<Request, Response = Response<B>, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    Serve {
        listener,
        connection_builder: Hyper::default(),
        make_service,
        _marker: PhantomData,
    }
}

/// Future returned by [`serve`].
#[cfg(feature = "tokio")]
#[must_use = "futures must be awaited or polled"]
pub struct Serve<L, C, M, S, B> {
    listener: L,
    connection_builder: C,
    make_service: M,
    _marker: PhantomData<fn(B) -> S>,
}

#[cfg(feature = "tokio")]
impl<L, C, M, S, B> Serve<L, C, M, S, B>
where
    L: Listener,
{
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
    ///
    /// # Return Value
    ///
    /// Similarly to [`serve`], although this future resolves to `io::Result<()>`, it will never
    /// error. It returns `Ok(())` only after the `signal` future completes.
    pub fn with_graceful_shutdown<F>(self, signal: F) -> WithGracefulShutdown<L, C, M, S, F, B>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        WithGracefulShutdown {
            listener: self.listener,
            connection_builder: self.connection_builder,
            make_service: self.make_service,
            signal,
            _marker: PhantomData,
        }
    }

    /// Returns the local address this server is bound to.
    pub fn local_addr(&self) -> io::Result<L::Addr> {
        self.listener.local_addr()
    }
}

#[cfg(feature = "tokio")]
impl<L, C, M, S, B> Serve<L, C, M, S, B>
where
    L: Listener,
    L::Addr: Debug,
    M: for<'a> Service<IncomingStream<'a, L>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a, L>>>::Future: Send,
    S: Service<Request, Response = Response<B>, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    /// Serve with a custom [`ConnectionBuilder`] implementation.
    ///
    /// # Example
    ///
    /// ```rust
    /// # async {
    /// # use axum::Router;
    /// # use axum::serve::{Hyper, serve};
    /// let connection_builder = Hyper::default();
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// serve(listener, Router::new()).with_connection_builder(connection_builder).await.unwrap();
    /// # };
    /// ```
    pub fn with_connection_builder<C2>(self, connection_builder: C2) -> Serve<L, C2, M, S, B>
    where
        C2: ConnectionBuilder<L::Io, S> + Send + 'static,
    {
        Serve {
            listener: self.listener,
            connection_builder,
            make_service: self.make_service,
            _marker: PhantomData,
        }
    }

    async fn run(self) -> !
    where
        C: ConnectionBuilder<L::Io, S> + Send + 'static,
    {
        let Self {
            mut listener,
            connection_builder,
            mut make_service,
            _marker,
        } = self;

        let (_close_tx, close_rx) = watch::channel(());

        loop {
            let (io, remote_addr) = listener.accept().await;
            handle_connection(
                &mut make_service,
                &close_rx,
                io,
                remote_addr,
                connection_builder.clone(),
            )
            .await;
        }
    }
}

#[cfg(feature = "tokio")]
impl<L, C, M, S, B> Debug for Serve<L, C, M, S, B>
where
    L: Debug,
    C: Debug,
    M: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            listener,
            connection_builder,
            make_service,
            _marker: _,
        } = self;

        f.debug_struct("Serve")
            .field("listener", listener)
            .field("connection_builder", connection_builder)
            .field("make_service", make_service)
            .finish()
    }
}

#[cfg(feature = "tokio")]
impl<L, C, M, S, B> IntoFuture for Serve<L, C, M, S, B>
where
    L: Listener,
    L::Addr: Debug,
    C: ConnectionBuilder<L::Io, S> + Send + 'static,
    M: for<'a> Service<IncomingStream<'a, L>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a, L>>>::Future: Send,
    S: Service<Request, Response = Response<B>, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Output = io::Result<()>;
    type IntoFuture = private::ServeFuture;

    fn into_future(self) -> Self::IntoFuture {
        private::ServeFuture(Box::pin(async move { self.run().await }))
    }
}

/// Serve future with graceful shutdown enabled.
#[cfg(feature = "tokio")]
#[must_use = "futures must be awaited or polled"]
pub struct WithGracefulShutdown<L, C, M, S, F, B> {
    listener: L,
    connection_builder: C,
    make_service: M,
    signal: F,
    _marker: PhantomData<fn(B) -> S>,
}

#[cfg(feature = "tokio")]
impl<L, C, M, S, F, B> WithGracefulShutdown<L, C, M, S, F, B>
where
    L: Listener,
{
    /// Returns the local address this server is bound to.
    pub fn local_addr(&self) -> io::Result<L::Addr> {
        self.listener.local_addr()
    }
}

#[cfg(feature = "tokio")]
impl<L, C, M, S, F, B> WithGracefulShutdown<L, C, M, S, F, B>
where
    L: Listener,
    L::Addr: Debug,
    C: ConnectionBuilder<L::Io, S> + Send + 'static,
    M: for<'a> Service<IncomingStream<'a, L>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a, L>>>::Future: Send,
    S: Service<Request, Response = Response<B>, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    F: Future<Output = ()> + Send + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    async fn run(self) {
        let Self {
            mut listener,
            mut make_service,
            signal,
            mut connection_builder,
            _marker,
        } = self;

        let (signal_tx, signal_rx) = watch::channel(());
        tokio::spawn(async move {
            signal.await;
            trace!("received graceful shutdown signal. Telling tasks to shutdown");
            drop(signal_rx);
        });

        let (close_tx, close_rx) = watch::channel(());

        loop {
            use futures_util::future::{select, Either};

            match select(pin!(listener.accept()), pin!(signal_tx.closed())).await {
                Either::Left(((io, remote_addr), _)) => {
                    handle_connection(
                        &mut make_service,
                        &close_rx,
                        io,
                        remote_addr,
                        connection_builder.clone(),
                    )
                    .await;
                }
                Either::Right(((), _)) => {
                    connection_builder.graceful_shutdown();
                    trace!("signal received, not accepting new connections");
                    break;
                }
            }
        }

        drop(close_rx);
        drop(listener);

        trace!(
            "waiting for {} task(s) to finish",
            close_tx.receiver_count()
        );
        close_tx.closed().await;
    }
}

#[cfg(feature = "tokio")]
impl<L, C, M, S, F, B> Debug for WithGracefulShutdown<L, C, M, S, F, B>
where
    L: Debug,
    C: Debug,
    M: Debug,
    S: Debug,
    F: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            listener,
            connection_builder,
            make_service,
            signal,
            _marker: _,
        } = self;

        f.debug_struct("WithGracefulShutdown")
            .field("listener", listener)
            .field("connection_builder", connection_builder)
            .field("make_service", make_service)
            .field("signal", signal)
            .finish()
    }
}

#[cfg(feature = "tokio")]
impl<L, C, M, S, F, B> IntoFuture for WithGracefulShutdown<L, C, M, S, F, B>
where
    L: Listener,
    L::Addr: Debug,
    C: ConnectionBuilder<L::Io, S> + Send + 'static,
    M: for<'a> Service<IncomingStream<'a, L>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a, L>>>::Future: Send,
    S: Service<Request, Response = Response<B>, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    F: Future<Output = ()> + Send + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Output = io::Result<()>;
    type IntoFuture = private::ServeFuture;

    fn into_future(self) -> Self::IntoFuture {
        private::ServeFuture(Box::pin(async move {
            self.run().await;
            Ok(())
        }))
    }
}

async fn handle_connection<L, M, S, B, C>(
    make_service: &mut M,
    close_rx: &watch::Receiver<()>,
    io: <L as Listener>::Io,
    remote_addr: <L as Listener>::Addr,
    mut connection_builder: C,
) where
    L: Listener,
    L::Addr: Debug,
    C: ConnectionBuilder<L::Io, S> + Send + 'static,
    M: for<'a> Service<IncomingStream<'a, L>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a, L>>>::Future: Send,
    S: Service<Request, Response = Response<B>, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    trace!("connection {remote_addr:?} accepted");

    make_service
        .ready()
        .await
        .unwrap_or_else(|err| match err {});

    let tower_service = make_service
        .call(IncomingStream {
            io: &io,
            remote_addr,
        })
        .await
        .unwrap_or_else(|err| match err {});

    let close_rx = close_rx.clone();

    tokio::spawn(async move {
        let connection = connection_builder.build_connection(io, tower_service);

        let mut connection = pin!(connection);

        let connection_future = poll_fn(|cx| connection.as_mut().poll_connection(cx));

        #[allow(unused_variables)] // Without tracing, the binding is unused.
        if let Err(err) = connection_future.await {
            trace!(error = debug(err), "failed to serve connection");
        }

        drop(close_rx);
    });
}

/// An incoming stream.
///
/// Used with [`serve`] and [`IntoMakeServiceWithConnectInfo`].
///
/// [`IntoMakeServiceWithConnectInfo`]: crate::extract::connect_info::IntoMakeServiceWithConnectInfo
#[derive(Debug)]
pub struct IncomingStream<'a, L>
where
    L: Listener,
{
    io: &'a L::Io,
    remote_addr: L::Addr,
}

impl<L> IncomingStream<'_, L>
where
    L: Listener,
{
    /// Get a reference to the inner IO type.
    pub fn io(&self) -> &L::Io {
        self.io
    }

    /// Returns the remote address that this stream is bound to.
    pub fn remote_addr(&self) -> &L::Addr {
        &self.remote_addr
    }
}

mod private {
    use std::{
        future::Future,
        io,
        pin::Pin,
        task::{Context, Poll},
    };

    pub struct ServeFuture(pub(super) futures_core::future::BoxFuture<'static, io::Result<()>>);

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

#[cfg(test)]
mod tests {
    use std::{
        future::{pending, IntoFuture as _},
        net::{IpAddr, Ipv4Addr},
        time::Duration,
    };

    use axum_core::{body::Body, extract::Request};
    use http::{Response, StatusCode};
    use hyper_util::rt::TokioIo;
    #[cfg(unix)]
    use tokio::net::UnixListener;
    use tokio::{
        io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
        net::TcpListener,
    };
    use tower::ServiceBuilder;

    #[cfg(unix)]
    use super::IncomingStream;
    use super::{serve, Listener};
    #[cfg(unix)]
    use crate::extract::connect_info::Connected;
    use crate::{
        body::to_bytes,
        handler::{Handler, HandlerWithoutStateExt},
        routing::get,
        serve::{Connection, ConnectionBuilder, ListenerExt},
        Router, ServiceExt,
    };

    struct ReadyListener<T>(Option<T>);

    impl<T> Listener for ReadyListener<T>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        type Io = T;
        type Addr = ();

        async fn accept(&mut self) -> (Self::Io, Self::Addr) {
            match self.0.take() {
                Some(server) => (server, ()),
                None => std::future::pending().await,
            }
        }

        fn local_addr(&self) -> io::Result<Self::Addr> {
            Ok(())
        }
    }

    #[allow(dead_code, unused_must_use)]
    async fn if_it_compiles_it_works() {
        #[derive(Clone, Debug)]
        struct UdsConnectInfo;

        #[cfg(unix)]
        impl Connected<IncomingStream<'_, UnixListener>> for UdsConnectInfo {
            fn connect_info(_stream: IncomingStream<'_, UnixListener>) -> Self {
                Self
            }
        }

        let router: Router = Router::new();

        let addr = "0.0.0.0:0";

        let tcp_nodelay_listener = || async {
            TcpListener::bind(addr).await.unwrap().tap_io(|tcp_stream| {
                if let Err(err) = tcp_stream.set_nodelay(true) {
                    eprintln!("failed to set TCP_NODELAY on incoming connection: {err:#}");
                }
            })
        };

        // router
        serve(TcpListener::bind(addr).await.unwrap(), router.clone());
        serve(tcp_nodelay_listener().await, router.clone())
            .await
            .unwrap();
        #[cfg(unix)]
        serve(UnixListener::bind("").unwrap(), router.clone());

        serve(
            TcpListener::bind(addr).await.unwrap(),
            router.clone().into_make_service(),
        );
        serve(
            tcp_nodelay_listener().await,
            router.clone().into_make_service(),
        );
        #[cfg(unix)]
        serve(
            UnixListener::bind("").unwrap(),
            router.clone().into_make_service(),
        );

        serve(
            TcpListener::bind(addr).await.unwrap(),
            router
                .clone()
                .into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        serve(
            tcp_nodelay_listener().await,
            router
                .clone()
                .into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        #[cfg(unix)]
        serve(
            UnixListener::bind("").unwrap(),
            router.into_make_service_with_connect_info::<UdsConnectInfo>(),
        );

        // method router
        serve(TcpListener::bind(addr).await.unwrap(), get(handler));
        serve(tcp_nodelay_listener().await, get(handler));
        #[cfg(unix)]
        serve(UnixListener::bind("").unwrap(), get(handler));

        serve(
            TcpListener::bind(addr).await.unwrap(),
            get(handler).into_make_service(),
        );
        serve(
            tcp_nodelay_listener().await,
            get(handler).into_make_service(),
        );
        #[cfg(unix)]
        serve(
            UnixListener::bind("").unwrap(),
            get(handler).into_make_service(),
        );

        serve(
            TcpListener::bind(addr).await.unwrap(),
            get(handler).into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        serve(
            tcp_nodelay_listener().await,
            get(handler).into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        #[cfg(unix)]
        serve(
            UnixListener::bind("").unwrap(),
            get(handler).into_make_service_with_connect_info::<UdsConnectInfo>(),
        );

        // handler
        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_service(),
        );
        serve(tcp_nodelay_listener().await, handler.into_service());
        #[cfg(unix)]
        serve(UnixListener::bind("").unwrap(), handler.into_service());

        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.with_state(()),
        );
        serve(tcp_nodelay_listener().await, handler.with_state(()));
        #[cfg(unix)]
        serve(UnixListener::bind("").unwrap(), handler.with_state(()));

        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_make_service(),
        );
        serve(tcp_nodelay_listener().await, handler.into_make_service());
        #[cfg(unix)]
        serve(UnixListener::bind("").unwrap(), handler.into_make_service());

        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        serve(
            tcp_nodelay_listener().await,
            handler.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        #[cfg(unix)]
        serve(
            UnixListener::bind("").unwrap(),
            handler.into_make_service_with_connect_info::<UdsConnectInfo>(),
        );
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

    #[tokio::test(start_paused = true)]
    async fn test_with_graceful_shutdown_request_header_timeout() {
        for (timeout, req) in [
            // Idle connections (between requests) are closed immediately
            // when a graceful shutdown is triggered.
            (0, ""),                       // idle before request sent
            (0, "GET / HTTP/1.1\r\n\r\n"), // idle after complete exchange
            // hyper times stalled request lines/headers out after 30 sec,
            // after which the graceful shutdown can be completed.
            (30, "GET / HT"),                   // stall during request line
            (30, "GET / HTTP/1.0\r\nAccept: "), // stall during request headers
        ] {
            let (mut client, server) = io::duplex(1024);
            client.write_all(req.as_bytes()).await.unwrap();

            let server_task = async {
                serve(ReadyListener(Some(server)), Router::new())
                    .with_graceful_shutdown(tokio::time::sleep(Duration::from_secs(1)))
                    .await
                    .unwrap();
            };

            tokio::time::timeout(Duration::from_secs(timeout + 2), server_task)
                .await
                .expect("server_task didn't exit in time");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_hyper_header_read_timeout_is_enabled() {
        let header_read_timeout_default = 30;
        for req in [
            "GET / HT",                   // stall during request line
            "GET / HTTP/1.0\r\nAccept: ", // stall during request headers
        ] {
            let (mut client, server) = io::duplex(1024);
            client.write_all(req.as_bytes()).await.unwrap();

            let server_task = async {
                serve(ReadyListener(Some(server)), Router::new())
                    .await
                    .unwrap();
            };

            let wait_for_server_to_close_conn = async {
                tokio::time::timeout(
                    Duration::from_secs(header_read_timeout_default + 1),
                    client.read_to_end(&mut Vec::new()),
                )
                .await
                .expect("timeout: server didn't close connection in time")
                .expect("read_to_end");
            };

            tokio::select! {
                _ = server_task => unreachable!(),
                _ = wait_for_server_to_close_conn => (),
            };
        }
    }

    #[test]
    fn into_future_outside_tokio() {
        let router: Router = Router::new();
        let addr = "0.0.0.0:0";

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let listener = rt.block_on(tokio::net::TcpListener::bind(addr)).unwrap();

        // Call Serve::into_future outside of a tokio context. This used to panic.
        _ = serve(listener, router).into_future();
    }

    #[crate::test]
    async fn serving_on_custom_io_type() {
        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        let app = Router::new().route("/", get(|| async { "Hello, World!" }));

        tokio::spawn(serve(listener, app).into_future());

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().body(Body::empty()).unwrap();

        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = Body::new(response.into_body());
        let body = to_bytes(body, usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body, "Hello, World!");
    }

    #[crate::test]
    async fn serving_with_custom_body_type() {
        struct CustomBody;
        impl http_body::Body for CustomBody {
            type Data = bytes::Bytes;
            type Error = std::convert::Infallible;
            fn poll_frame(
                self: std::pin::Pin<&mut Self>,
                _cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>>
            {
                #![allow(clippy::unreachable)] // The implementation is not used, we just need to provide one.
                unreachable!();
            }
        }

        let app = ServiceBuilder::new()
            .layer_fn(|_| tower::service_fn(|_| std::future::ready(Ok(Response::new(CustomBody)))))
            .service(Router::<()>::new().route("/hello", get(|| async {})));
        let addr = "0.0.0.0:0";

        _ = serve(
            TcpListener::bind(addr).await.unwrap(),
            app.into_make_service(),
        );
    }

    #[crate::test]
    async fn serving_without_hyper() {
        #[derive(Clone)]
        struct OkGenerator;

        impl<Io: AsyncWrite + Unpin + Send + 'static, S> ConnectionBuilder<Io, S> for OkGenerator {
            fn build_connection(&mut self, mut io: Io, _service: S) -> impl Connection {
                Box::pin(async move {
                    io.write_all(b"OK").await?;
                    Ok(())
                })
            }

            fn graceful_shutdown(&mut self) {}
        }

        let (mut client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        let app = Router::new().route("/", get(|| async { "Hello, World!" }));

        tokio::spawn(
            serve(listener, app)
                .with_connection_builder(OkGenerator)
                .into_future(),
        );

        let mut buf = [0u8; 2];
        client.read_exact(&mut buf).await.unwrap();

        assert_eq!(&buf, b"OK");
    }
}
