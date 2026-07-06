//! Serve `axum` (and other [`tower`]-based) services with [`hyper`].
//!
//! This crate provides the [`serve`] function and the [`Listener`] trait that power
//! `axum::serve`. It intentionally does not depend on the `axum` crate, so libraries that
//! want to implement a custom [`Listener`] (for example, a TLS-terminating listener) can
//! depend on `axum-serve` directly instead of pulling in all of `axum`.
//!
//! Most users should not depend on this crate directly and should instead use the
//! re-exports available at `axum::serve`.

#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(not(test), warn(clippy::print_stdout, clippy::dbg_macro))]

#[macro_use]
mod macros;

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
use std::{
    convert::Infallible,
    error::Error as StdError,
    fmt::Debug,
    future::{Future, IntoFuture},
    io,
    marker::PhantomData,
    pin::pin,
    sync::Arc,
};

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
use axum_core::{body::Body, extract::Request, response::Response};
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
use futures_util::FutureExt;
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
use http_body::Body as HttpBody;
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
use hyper::body::Incoming;
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
use hyper_util::rt::TokioIo;
#[cfg(all(feature = "tokio", feature = "http1"))]
use hyper_util::rt::TokioTimer;
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
use hyper_util::{server::conn::auto::Builder, service::TowerToHyperService};
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
use tokio::{sync::watch, task::JoinHandle};
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
use tower::ServiceExt as _;
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
use tower_service::Service;

mod listener;

pub use self::listener::{ConnLimiter, ConnLimiterIo, Listener, ListenerExt, TapIo};

/// Serve the service with the supplied listener.
///
/// This method of running a service is intentionally simple and doesn't support much configuration.
/// hyper's default configuration applies (including [timeouts]); use hyper or hyper-util if you
/// need more control. You can supply a custom [`Executor`] via [`Serve::with_executor`] to
/// control how connection tasks are spawned.
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
/// axum::serve(listener, router).await;
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
/// axum::serve(listener, router).await;
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
/// axum::serve(listener, handler.into_make_service()).await;
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
/// [`Router`]: https://docs.rs/axum/latest/axum/struct.Router.html
/// [`Router::into_make_service_with_connect_info`]: https://docs.rs/axum/latest/axum/struct.Router.html#method.into_make_service_with_connect_info
/// [`MethodRouter`]: https://docs.rs/axum/latest/axum/routing/struct.MethodRouter.html
/// [`MethodRouter::into_make_service_with_connect_info`]: https://docs.rs/axum/latest/axum/routing/struct.MethodRouter.html#method.into_make_service_with_connect_info
/// [`Handler`]: https://docs.rs/axum/latest/axum/handler/trait.Handler.html
/// [`HandlerWithoutStateExt::into_make_service_with_connect_info`]: https://docs.rs/axum/latest/axum/handler/trait.HandlerWithoutStateExt.html#method.into_make_service_with_connect_info
/// [`HandlerService::into_make_service_with_connect_info`]: https://docs.rs/axum/latest/axum/handler/struct.HandlerService.html#method.into_make_service_with_connect_info
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
pub fn serve<L, M, S, B>(listener: L, make_service: M) -> Serve<L, M, S, B, TokioExecutor>
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
        make_service,
        executor: TokioExecutor,
        _marker: PhantomData,
    }
}

/// A Tokio executor used by [`serve`] to spawn connection tasks, graceful shutdown
/// tasks, and hyper's internal tasks (e.g. HTTP/2 connection management).
///
/// The default executor is [`TokioExecutor`], which simply calls to
/// [`tokio::spawn`]. A custom implementation can be provided to wrap
/// spawned tasks, e.g. to add tracing or telemetry.
///
/// Spawned futures rely on Tokio primitives internally, so the executor
/// must run them within a Tokio runtime context (e.g. via [`tokio::spawn`]).
///
/// # Example
///
/// An executor that wraps every spawned task in a [`tracing`] span.
///
/// ```
/// use std::future::Future;
/// use axum::serve::Executor;
/// use tokio::task::JoinHandle;
/// use tracing::Instrument;
///
/// #[derive(Clone)]
/// struct InstrumentedExecutor;
///
/// impl Executor for InstrumentedExecutor {
///     fn execute<Fut>(&self, fut: Fut) -> JoinHandle<Fut::Output>
///     where
///         Fut: Future + Send + 'static,
///         Fut::Output: Send + 'static,
///     {
///         let span = tracing::info_span!("axum.serve.task");
///         tokio::spawn(fut.instrument(span))
///     }
/// }
/// ```
///
/// If your executor is expensive to clone, wrap it in an `Arc`.
/// A blanket implementation is provided for `Arc<T>` where `T: Executor`.
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
pub trait Executor: Clone + Send + Sync + 'static {
    /// Execute a task.
    fn execute<Fut>(&self, fut: Fut) -> JoinHandle<Fut::Output>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static;
}

/// The default executor, which uses [`tokio::spawn`].
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
#[derive(Clone, Debug)]
pub struct TokioExecutor;

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl Executor for TokioExecutor {
    fn execute<Fut>(&self, fut: Fut) -> JoinHandle<Fut::Output>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
    {
        tokio::spawn(fut)
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<T> Executor for Arc<T>
where
    T: Executor,
{
    fn execute<Fut>(&self, fut: Fut) -> JoinHandle<Fut::Output>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
    {
        self.as_ref().execute(fut)
    }
}

/// Future returned by [`serve`].
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
#[must_use = "futures must be awaited or polled"]
pub struct Serve<L, M, S, B, E = TokioExecutor> {
    listener: L,
    make_service: M,
    executor: E,
    _marker: PhantomData<fn(B) -> S>,
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<L, M, S, B, E> Serve<L, M, S, B, E>
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
    ///     .await;
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
    pub fn with_graceful_shutdown<F>(self, signal: F) -> WithGracefulShutdown<L, M, S, F, B, E>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        WithGracefulShutdown {
            listener: self.listener,
            make_service: self.make_service,
            executor: self.executor,
            signal,
            _marker: PhantomData,
        }
    }

    /// Returns the local address this server is bound to.
    pub fn local_addr(&self) -> io::Result<L::Addr> {
        self.listener.local_addr()
    }

    /// Provide a custom [`Executor`] to use for spawning connection tasks and
    /// hyper's internal tasks (e.g. HTTP/2).
    ///
    /// The default is [`TokioExecutor`]. See the [`Executor`] docs for how to
    /// implement a custom one.
    ///
    /// This method can be called before or after [`with_graceful_shutdown`].
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{Router, routing::get, serve::Executor};
    /// # use std::future::Future;
    /// # use tokio::task::JoinHandle;
    /// #
    /// # #[derive(Clone)]
    /// # struct MyExecutor;
    /// #
    /// # impl Executor for MyExecutor {
    /// #     fn execute<Fut>(&self, fut: Fut) -> JoinHandle<Fut::Output>
    /// #     where
    /// #         Fut: Future + Send + 'static,
    /// #         Fut::Output: Send + 'static,
    /// #     {
    /// #         tokio::spawn(fut)
    /// #     }
    /// # }
    /// #
    /// # async {
    /// let router = Router::new().route("/", get(|| async { "Hello, World!" }));
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    ///
    /// axum::serve(listener, router)
    ///     .with_executor(MyExecutor)
    ///     .await;
    /// # };
    /// ```
    ///
    /// [`with_graceful_shutdown`]: Serve::with_graceful_shutdown
    pub fn with_executor<E2>(self, executor: E2) -> Serve<L, M, S, B, E2>
    where
        E2: Executor,
    {
        Serve {
            listener: self.listener,
            make_service: self.make_service,
            executor,
            _marker: PhantomData,
        }
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<L, M, S, B, E> Serve<L, M, S, B, E>
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
    E: Executor,
{
    async fn run(self) -> ! {
        let Self {
            mut listener,
            mut make_service,
            executor,
            _marker,
        } = self;

        let (signal_tx, _signal_rx) = watch::channel(());
        let (_close_tx, close_rx) = watch::channel(());

        loop {
            let (io, remote_addr) = listener.accept().await;
            handle_connection(
                &mut make_service,
                &signal_tx,
                &close_rx,
                io,
                remote_addr,
                &executor,
            )
            .await;
        }
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<L, M, S, B, E> Debug for Serve<L, M, S, B, E>
where
    L: Debug + 'static,
    M: Debug,
    E: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            listener,
            make_service,
            executor,
            _marker: _,
        } = self;

        let mut s = f.debug_struct("Serve");
        s.field("listener", listener)
            .field("make_service", make_service)
            .field("executor", executor);

        s.finish()
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<L, M, S, B, E> IntoFuture for Serve<L, M, S, B, E>
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
    E: Executor,
{
    type Output = Infallible;
    type IntoFuture = private::ServeFuture;

    fn into_future(self) -> Self::IntoFuture {
        private::ServeFuture(Box::pin(async move { self.run().await }))
    }
}

/// Serve future with graceful shutdown enabled.
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
#[must_use = "futures must be awaited or polled"]
pub struct WithGracefulShutdown<L, M, S, F, B, E = TokioExecutor> {
    listener: L,
    make_service: M,
    executor: E,
    signal: F,
    _marker: PhantomData<fn(B) -> S>,
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<L, M, S, F, B, E> WithGracefulShutdown<L, M, S, F, B, E>
where
    L: Listener,
{
    /// Returns the local address this server is bound to.
    pub fn local_addr(&self) -> io::Result<L::Addr> {
        self.listener.local_addr()
    }

    /// Provide a custom [`Executor`] to use for spawning connection tasks and
    /// hyper's internal tasks (e.g. HTTP/2).
    ///
    /// See [`Serve::with_executor`] for details.
    pub fn with_executor<E2>(self, executor: E2) -> WithGracefulShutdown<L, M, S, F, B, E2>
    where
        E2: Executor,
    {
        WithGracefulShutdown {
            listener: self.listener,
            make_service: self.make_service,
            executor,
            signal: self.signal,
            _marker: PhantomData,
        }
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<L, M, S, F, B, E> WithGracefulShutdown<L, M, S, F, B, E>
where
    L: Listener,
    L::Addr: Debug,
    M: for<'a> Service<IncomingStream<'a, L>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a, L>>>::Future: Send,
    S: Service<Request, Response = Response<B>, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    F: Future<Output = ()> + Send + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: Executor,
{
    async fn run(self) {
        let Self {
            mut listener,
            mut make_service,
            executor,
            signal,
            _marker,
        } = self;

        let (signal_tx, signal_rx) = watch::channel(());
        executor.execute(async move {
            signal.await;
            trace!("received graceful shutdown signal. Telling tasks to shutdown");
            drop(signal_rx);
        });

        let (close_tx, close_rx) = watch::channel(());

        loop {
            let (io, remote_addr) = tokio::select! {
                conn = listener.accept() => conn,
                _ = signal_tx.closed() => {
                    trace!("signal received, not accepting new connections");
                    break;
                }
            };

            handle_connection(
                &mut make_service,
                &signal_tx,
                &close_rx,
                io,
                remote_addr,
                &executor,
            )
            .await;
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

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<L, M, S, F, B, E> Debug for WithGracefulShutdown<L, M, S, F, B, E>
where
    L: Debug + 'static,
    M: Debug,
    S: Debug,
    F: Debug,
    E: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            listener,
            make_service,
            executor: _,
            signal,
            _marker: _,
        } = self;

        f.debug_struct("WithGracefulShutdown")
            .field("listener", listener)
            .field("make_service", make_service)
            .field("signal", signal)
            .finish()
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<L, M, S, F, B, E> IntoFuture for WithGracefulShutdown<L, M, S, F, B, E>
where
    L: Listener,
    L::Addr: Debug,
    M: for<'a> Service<IncomingStream<'a, L>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a, L>>>::Future: Send,
    S: Service<Request, Response = Response<B>, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    F: Future<Output = ()> + Send + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: Executor,
{
    type Output = ();
    type IntoFuture = private::ServeFuture<()>;

    fn into_future(self) -> Self::IntoFuture {
        private::ServeFuture(Box::pin(async move { self.run().await }))
    }
}

/// Adapts axum's [`Executor`] to hyper's [`hyper::rt::Executor`].
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
#[derive(Clone)]
struct HyperExecutor<E>(E);

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<E, Fut> hyper::rt::Executor<Fut> for HyperExecutor<E>
where
    E: Executor,
    Fut: Future<Output = ()> + Send + 'static,
{
    fn execute(&self, fut: Fut) {
        drop(self.0.execute(fut));
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
async fn handle_connection<L, M, S, B, E>(
    make_service: &mut M,
    signal_tx: &watch::Sender<()>,
    close_rx: &watch::Receiver<()>,
    io: <L as Listener>::Io,
    remote_addr: <L as Listener>::Addr,
    executor: &E,
) where
    L: Listener,
    L::Addr: Debug,
    M: for<'a> Service<IncomingStream<'a, L>, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<IncomingStream<'a, L>>>::Future: Send,
    S: Service<Request, Response = Response<B>, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: Executor,
{
    let io = TokioIo::new(io);

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
        .unwrap_or_else(|err| match err {})
        .map_request(|req: Request<Incoming>| req.map(Body::new));

    let hyper_service = TowerToHyperService::new(tower_service);
    let signal_tx = signal_tx.clone();
    let close_rx = close_rx.clone();

    let hyper_executor = HyperExecutor(executor.clone());
    executor.execute(async move {
        #[allow(unused_mut)]
        let mut builder = Builder::new(hyper_executor);

        // Enable Hyper's default HTTP/1 request header timeout.
        #[cfg(feature = "http1")]
        builder.http1().timer(TokioTimer::new());

        // CONNECT protocol needed for HTTP/2 websockets
        #[cfg(feature = "http2")]
        builder.http2().enable_connect_protocol();

        let mut conn = pin!(builder.serve_connection_with_upgrades(io, hyper_service));
        let mut signal_closed = pin!(signal_tx.closed().fuse());

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

        drop(close_rx);
    });
}

/// An incoming stream.
///
/// Used with [`serve`] and [`IntoMakeServiceWithConnectInfo`].
///
/// [`IntoMakeServiceWithConnectInfo`]: https://docs.rs/axum/latest/axum/extract/connect_info/struct.IntoMakeServiceWithConnectInfo.html
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
#[derive(Debug)]
pub struct IncomingStream<'a, L>
where
    L: Listener,
{
    io: &'a TokioIo<L::Io>,
    remote_addr: L::Addr,
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
impl<L> IncomingStream<'_, L>
where
    L: Listener,
{
    /// Get a reference to the inner IO type.
    pub fn io(&self) -> &L::Io {
        self.io.inner()
    }

    /// Returns the remote address that this stream is bound to.
    pub fn remote_addr(&self) -> &L::Addr {
        &self.remote_addr
    }
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
mod private {
    use std::{
        convert::Infallible,
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };

    pub struct ServeFuture<T = Infallible>(pub(super) futures_core::future::BoxFuture<'static, T>);

    impl<T> Future for ServeFuture<T> {
        type Output = T;

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
        convert::Infallible,
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
        task::JoinHandle,
    };
    use tower::{make::Shared, service_fn};
    use tower_service::Service;

    use super::{serve, Listener, ListenerExt};

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

    /// A make-service which always responds with a fixed body.
    fn body_service(
        body: &'static str,
    ) -> Shared<
        impl Service<Request, Response = Response<Body>, Error = Infallible, Future = impl Send> + Clone,
    > {
        Shared::new(service_fn(move |_req: Request| async move {
            Ok::<_, Infallible>(Response::new(Body::from(body)))
        }))
    }

    /// A make-service which always responds with an empty (404-like) body, mirroring an
    /// empty `Router`.
    fn empty_service() -> Shared<
        impl Service<Request, Response = Response<Body>, Error = Infallible, Future = impl Send> + Clone,
    > {
        Shared::new(service_fn(|_req: Request| async {
            Ok::<_, Infallible>(
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::empty())
                    .unwrap(),
            )
        }))
    }

    #[allow(dead_code, unused_must_use)]
    async fn if_it_compiles_it_works() {
        let addr = "0.0.0.0:0";

        let tcp_nodelay_listener = || async {
            TcpListener::bind(addr).await.unwrap().tap_io(|tcp_stream| {
                if let Err(err) = tcp_stream.set_nodelay(true) {
                    eprintln!("failed to set TCP_NODELAY on incoming connection: {err:#}");
                }
            })
        };

        serve(TcpListener::bind(addr).await.unwrap(), body_service("hi"));
        serve(tcp_nodelay_listener().await, body_service("hi")).await;
        #[cfg(unix)]
        serve(UnixListener::bind("").unwrap(), body_service("hi"));

        // with_executor
        let exec = TestExecutor::new();
        serve(TcpListener::bind(addr).await.unwrap(), body_service("hi"))
            .with_executor(exec.clone());
        serve(TcpListener::bind(addr).await.unwrap(), body_service("hi"))
            .with_executor(exec.clone())
            .with_graceful_shutdown(std::future::pending());
        serve(TcpListener::bind(addr).await.unwrap(), body_service("hi"))
            .with_graceful_shutdown(std::future::pending())
            .with_executor(exec);
    }

    #[derive(Clone)]
    struct TestExecutor(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl TestExecutor {
        fn new() -> Self {
            Self(std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)))
        }

        fn count(&self) -> usize {
            self.0.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl super::Executor for TestExecutor {
        fn execute<Fut>(&self, fut: Fut) -> JoinHandle<Fut::Output>
        where
            Fut: std::future::Future + Send + 'static,
            Fut::Output: Send + 'static,
        {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tokio::spawn(fut)
        }
    }

    #[tokio::test]
    async fn test_serve_local_addr() {
        let addr = "0.0.0.0:0";

        let server = serve(TcpListener::bind(addr).await.unwrap(), body_service("hi"));
        let address = server.local_addr().unwrap();

        assert_eq!(address.ip(), IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
        assert_ne!(address.port(), 0);
    }

    #[tokio::test]
    async fn test_with_graceful_shutdown_local_addr() {
        let addr = "0.0.0.0:0";

        let server = serve(TcpListener::bind(addr).await.unwrap(), body_service("hi"))
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
                serve(ReadyListener(Some(server)), empty_service())
                    .with_graceful_shutdown(tokio::time::sleep(Duration::from_secs(1)))
                    .await;
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
                serve(ReadyListener(Some(server)), empty_service()).await;
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
        let addr = "0.0.0.0:0";

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let listener = rt.block_on(tokio::net::TcpListener::bind(addr)).unwrap();

        // Call Serve::into_future outside of a tokio context. This used to panic.
        _ = serve(listener, body_service("hi")).into_future();
    }

    #[tokio::test]
    async fn serving_on_custom_io_type() {
        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(serve(listener, body_service("Hello, World!")).into_future());

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().body(Body::empty()).unwrap();

        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = Body::new(response.into_body());
        let body = crate_test_helpers::to_bytes(body).await;
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body, "Hello, World!");
    }

    // Asserts the documented `with_graceful_shutdown` drain semantics: after the
    // signal fires, an already-in-flight request is allowed to run to completion
    // and only then does the `serve` future resolve. The existing
    // `test_with_graceful_shutdown_request_header_timeout` only covers stalled
    // requests being killed by hyper's header read timeout.
    #[tokio::test]
    async fn graceful_shutdown_completes_inflight_request() {
        use std::sync::Arc;

        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let started2 = started.clone();
        let release2 = release.clone();
        let app = Shared::new(service_fn(move |_req: Request| {
            let started = started2.clone();
            let release = release2.clone();
            async move {
                started.notify_one();
                release.notified().await;
                Ok::<_, Infallible>(Response::new(Body::from("done")))
            }
        }));

        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        let server_task = tokio::spawn(
            serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_rx.await.ok();
                })
                .into_future(),
        );

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let request_fut = tokio::spawn(async move { sender.send_request(request).await });

        // Wait until the handler is actually running.
        started.notified().await;

        // Signal graceful shutdown while the request is still in flight.
        shutdown_tx.send(()).unwrap();

        // Give the signal time to be observed by the accept loop. The server
        // must NOT have completed yet because the in-flight request is held.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !server_task.is_finished(),
            "serve resolved before in-flight request completed",
        );

        // Release the handler. The in-flight request should now succeed.
        release.notify_one();

        let response = request_fut.await.unwrap().unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = crate_test_helpers::to_bytes(Body::new(response.into_body())).await;
        assert_eq!(&body[..], b"done");

        // And only after the in-flight request finished does serve resolve.
        tokio::time::timeout(Duration::from_secs(2), server_task)
            .await
            .expect("serve future did not resolve after in-flight request finished")
            .unwrap();
    }

    // Asserts that `ListenerExt::tap_io` invokes its closure on every accepted
    // connection when used with `serve`. The sibling `ListenerExt::limit_connections`
    // has a direct unit test (in `listener::tests`); `tap_io` did not have
    // a runtime test, so its documented contract was only covered at the type level
    // by `if_it_compiles_it_works`.
    #[tokio::test]
    async fn tap_io_runs_on_each_accepted_connection() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        let count = Arc::new(AtomicUsize::new(0));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let counted = {
            let count = count.clone();
            listener.tap_io(move |_io| {
                count.fetch_add(1, Ordering::SeqCst);
            })
        };

        tokio::spawn(serve(counted, body_service("ok")).into_future());

        // Open two distinct TCP connections to force two accepts.
        for _ in 0..2 {
            let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            let io = TokioIo::new(stream);
            let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await.unwrap();
            let conn_handle = tokio::spawn(conn);

            let request = Request::builder().uri("/").body(Body::empty()).unwrap();
            let response = sender.send_request(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);

            drop(sender);
            let _ = conn_handle.await;
        }

        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn serving_with_custom_executor() {
        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        let executor = TestExecutor::new();
        tokio::spawn(
            serve(listener, body_service("Hello, World!"))
                .with_executor(executor.clone())
                .into_future(),
        );

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().body(Body::empty()).unwrap();

        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = Body::new(response.into_body());
        let body = crate_test_helpers::to_bytes(body).await;
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body, "Hello, World!");

        // One task per connection for HTTP/1.
        assert_eq!(executor.count(), 1);
    }

    #[tokio::test]
    #[cfg(feature = "http2")]
    async fn serving_with_custom_executor_http2() {
        use hyper_util::rt::TokioExecutor;

        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        let executor = TestExecutor::new();
        tokio::spawn(
            serve(listener, body_service("Hello, World!"))
                .with_executor(executor.clone())
                .into_future(),
        );

        let io = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http2::Builder::new(TokioExecutor::new())
            .handshake(io)
            .await
            .unwrap();
        tokio::spawn(conn);

        let request = Request::builder().body(Body::empty()).unwrap();

        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = Body::new(response.into_body());
        let body = crate_test_helpers::to_bytes(body).await;
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body, "Hello, World!");

        // Two tasks: axum-serve's connection, and hyper's internal HTTP/2 task.
        assert_eq!(executor.count(), 2);
    }

    #[tokio::test]
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

        let app = Shared::new(service_fn(|_req: Request| {
            std::future::ready(Ok::<_, Infallible>(Response::new(CustomBody)))
        }));
        let addr = "0.0.0.0:0";

        _ = serve(TcpListener::bind(addr).await.unwrap(), app);
    }

    /// Minimal replacement for `axum::body::to_bytes` so these tests don't need to
    /// depend on `axum` itself.
    mod crate_test_helpers {
        use axum_core::body::Body;
        use bytes::Bytes;
        use http_body_util::BodyExt;

        pub(super) async fn to_bytes(body: Body) -> Bytes {
            body.collect().await.unwrap().to_bytes()
        }
    }
}
