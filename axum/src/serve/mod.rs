//! Serve services.

use std::{
    convert::Infallible,
    error::Error as StdError,
    fmt::Debug,
    future::{Future, IntoFuture, Pending},
    hash::{BuildHasher, Hasher},
    io,
    marker::PhantomData,
    pin::pin,
    sync::Arc,
    time::Duration,
};

use axum_core::{body::Body, extract::Request, response::Response};
use futures_util::{
    future::{select, Either},
    FutureExt,
};
use http_body::Body as HttpBody;
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
#[cfg(feature = "http1")]
use hyper_util::rt::TokioTimer;
#[cfg(any(feature = "http1", feature = "http2"))]
use hyper_util::{server::conn::auto::Builder, service::TowerToHyperService};
use tokio::{sync::watch, task::JoinHandle};
use tower::ServiceExt as _;
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
/// [`Router`]: crate::Router
/// [`Router::into_make_service_with_connect_info`]: crate::Router::into_make_service_with_connect_info
/// [`MethodRouter`]: crate::routing::MethodRouter
/// [`MethodRouter::into_make_service_with_connect_info`]: crate::routing::MethodRouter::into_make_service_with_connect_info
/// [`Handler`]: crate::handler::Handler
/// [`HandlerWithoutStateExt::into_make_service_with_connect_info`]: crate::handler::HandlerWithoutStateExt::into_make_service_with_connect_info
/// [`HandlerService::into_make_service_with_connect_info`]: crate::handler::HandlerService::into_make_service_with_connect_info
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
        connection_lifetime_limits: ConnectionLifetimeLimits::default(),
        _marker: PhantomData,
    }
}

/// Per-connection limits applied by [`serve`], used to bound the lifetime of
/// individual connections.
///
/// Closing connections after a bounded lifetime pressures clients to establish
/// *new* connections, which is useful behind a load balancer (e.g. a Kubernetes
/// `Service`) that round-robins new connections across the current set of
/// backends: without rotation, a client's connection pool keeps sending work to
/// whichever backends it first connected to, even after the pool has scaled up.
/// It also bounds the worst case when a client's connection pool has no
/// rotation of its own.
///
/// The mechanism differs by protocol but the knobs are the same:
///
/// - **HTTP/1**: the next response gets a `Connection: close` header and the
///   connection is closed once the in-flight request finishes.
/// - **HTTP/2**: a `GOAWAY` is sent, so new streams are refused
///   while in-flight streams are allowed to finish.
///
/// In both cases in-flight work is waited on for as long as it takes, unless
/// [`max_connection_age_grace`] is set: once the grace period elapses the
/// connection is closed even if a request is still in flight. See
/// [`max_connection_age_grace`] for the trade-off.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use axum::{Router, routing::get, serve::ConnectionLifetimeLimits};
///
/// # async {
/// let router = Router::new().route("/", get(|| async { "Hello, World!" }));
/// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
///
/// let limits = ConnectionLifetimeLimits::new()
///     // Stop accepting new requests on a connection after this long.
///     .max_connection_age(Duration::from_secs(10 * 60))
///     // Random per-connection jitter added to the age, to avoid synchronized
///     // reconnect storms when many connections were established at once.
///     .max_connection_age_jitter(Duration::from_secs(60))
///     // Hard cap on how long to wait for in-flight work after the age limit
///     // fires before forcibly closing.
///     .max_connection_age_grace(Duration::from_secs(30));
///
/// axum::serve(listener, router)
///     .connection_lifetime_limits(limits)
///     .await;
/// # };
/// ```
///
/// [`max_connection_age_grace`]: ConnectionLifetimeLimits::max_connection_age_grace
#[derive(Clone, Copy, Debug, Default)]
#[must_use]
pub struct ConnectionLifetimeLimits {
    max_connection_age: Option<Duration>,
    max_connection_age_jitter: Duration,
    max_connection_age_grace: Option<Duration>,
}

impl ConnectionLifetimeLimits {
    /// Create a new [`ConnectionLifetimeLimits`] with no limits set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a cap on how long a connection keeps accepting new requests.
    ///
    /// Once a connection has been open for this long, a graceful shutdown of
    /// that connection is started: HTTP/1 connections close after the in-flight
    /// request completes (sending `Connection: close`), and HTTP/2 connections
    /// send a `GOAWAY`, refusing new streams while letting in-flight ones finish
    /// (bounded by [`max_connection_age_grace`] if set).
    ///
    /// Consider also setting [`max_connection_age_jitter`] to avoid all
    /// connections opened around the same time tearing down simultaneously.
    ///
    /// [`max_connection_age_grace`]: ConnectionLifetimeLimits::max_connection_age_grace
    /// [`max_connection_age_jitter`]: ConnectionLifetimeLimits::max_connection_age_jitter
    pub fn max_connection_age(mut self, age: Duration) -> Self {
        self.max_connection_age = Some(age);
        self
    }

    /// Set the maximum random jitter added to [`max_connection_age`].
    ///
    /// Each connection adds a random duration in `[0, jitter]` to its age limit.
    /// This is important for avoiding synchronized reconnect storms when many
    /// connections were established at the same time (e.g. right after a
    /// deploy): without it, every connection opened in the same instant tears
    /// down in the same instant once the age limit elapses.
    ///
    /// Defaults to `Duration::ZERO`, i.e. no jitter. Has no effect unless
    /// [`max_connection_age`] is also set.
    ///
    /// [`max_connection_age`]: ConnectionLifetimeLimits::max_connection_age
    pub fn max_connection_age_jitter(mut self, jitter: Duration) -> Self {
        self.max_connection_age_jitter = jitter;
        self
    }

    /// Set a hard cap on how long to wait for in-flight work after
    /// [`max_connection_age`] fires before forcibly closing the connection.
    ///
    /// Without a grace period, [`max_connection_age`] only stops new requests:
    /// the server waits however long it takes for in-flight work to finish
    /// before closing the connection. Setting a grace period turns
    /// `max_connection_age` (+ jitter) + grace into a hard deadline: when it
    /// elapses the connection is closed *even if a request is still in flight*,
    /// and the client never receives a response for it. This applies to HTTP/1
    /// requests as well as HTTP/2 streams, so a handler that runs longer than
    /// the age limit plus the grace period will never complete successfully.
    /// Only set a grace period if bounding connection lifetime matters more
    /// than letting slow requests finish.
    ///
    /// This has no effect unless [`max_connection_age`] is also set.
    ///
    /// [`max_connection_age`]: ConnectionLifetimeLimits::max_connection_age
    pub fn max_connection_age_grace(mut self, grace: Duration) -> Self {
        self.max_connection_age_grace = Some(grace);
        self
    }
}

/// Returns a pseudo-random [`Duration`] in `[Duration::ZERO, max]`.
fn random_duration(max: Duration) -> Duration {
    if max.is_zero() {
        return Duration::ZERO;
    }

    // Each `RandomState` is seeded with fresh random keys, so hashing empty
    // input still yields a different value per call.
    let rand = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();

    let max_nanos = u64::try_from(max.as_nanos()).unwrap_or(u64::MAX);
    Duration::from_nanos(rand % max_nanos.saturating_add(1))
}

/// Sleeps for `duration`, or never completes if it's `None`.
///
/// Used to model an unset timer without having to poll a future that would
/// otherwise complete immediately.
fn sleep_or_pending(duration: Option<Duration>) -> Either<tokio::time::Sleep, Pending<()>> {
    match duration {
        Some(duration) => Either::Left(tokio::time::sleep(duration)),
        None => Either::Right(std::future::pending()),
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
    connection_lifetime_limits: ConnectionLifetimeLimits,
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
            connection_lifetime_limits: self.connection_lifetime_limits,
            signal,
            _marker: PhantomData,
        }
    }

    /// Returns the local address this server is bound to.
    pub fn local_addr(&self) -> io::Result<L::Addr> {
        self.listener.local_addr()
    }

    /// Apply per-connection [`ConnectionLifetimeLimits`], bounding the lifetime of
    /// individual connections.
    ///
    /// This is useful for forcing clients to rotate connections — see
    /// [`ConnectionLifetimeLimits`] for details and an example.
    ///
    /// This method can be called before or after [`with_graceful_shutdown`] and
    /// [`with_executor`].
    ///
    /// [`with_graceful_shutdown`]: Serve::with_graceful_shutdown
    /// [`with_executor`]: Serve::with_executor
    pub fn connection_lifetime_limits(mut self, limits: ConnectionLifetimeLimits) -> Self {
        self.connection_lifetime_limits = limits;
        self
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
            connection_lifetime_limits: self.connection_lifetime_limits,
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
            connection_lifetime_limits,
            _marker,
        } = self;

        let (_signal_tx, signal_rx) = watch::channel(());
        let (_close_tx, close_rx) = watch::channel(());

        loop {
            let (io, remote_addr) = listener.accept().await;
            handle_connection(
                &mut make_service,
                &signal_rx,
                &close_rx,
                io,
                remote_addr,
                &executor,
                connection_lifetime_limits,
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
            connection_lifetime_limits,
            _marker: _,
        } = self;

        let mut s = f.debug_struct("Serve");
        s.field("listener", listener)
            .field("make_service", make_service)
            .field("executor", executor)
            .field("connection_lifetime_limits", connection_lifetime_limits);

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
    connection_lifetime_limits: ConnectionLifetimeLimits,
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
            connection_lifetime_limits: self.connection_lifetime_limits,
            signal: self.signal,
            _marker: PhantomData,
        }
    }

    /// Apply per-connection [`ConnectionLifetimeLimits`], bounding the lifetime of
    /// individual connections.
    ///
    /// See [`Serve::connection_lifetime_limits`] and [`ConnectionLifetimeLimits`] for details.
    pub fn connection_lifetime_limits(mut self, limits: ConnectionLifetimeLimits) -> Self {
        self.connection_lifetime_limits = limits;
        self
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
            connection_lifetime_limits,
            signal,
            _marker,
        } = self;

        let (signal_tx, mut signal_rx) = watch::channel(());
        executor.execute(async move {
            signal.await;
            trace!("received graceful shutdown signal. Telling tasks to shutdown");
            drop(signal_tx);
        });

        let (close_tx, close_rx) = watch::channel(());

        loop {
            let (io, remote_addr) =
                match select(pin!(listener.accept()), pin!(signal_rx.changed())).await {
                    Either::Left((conn, _)) => conn,
                    Either::Right((Err(_), _)) => {
                        trace!("signal received, not accepting new connections");
                        break;
                    }
                    Either::Right((Ok(()), _)) => {
                        unreachable!("shutdown channel never sends values")
                    }
                };

            handle_connection(
                &mut make_service,
                &signal_rx,
                &close_rx,
                io,
                remote_addr,
                &executor,
                connection_lifetime_limits,
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
            connection_lifetime_limits,
            signal,
            _marker: _,
        } = self;

        f.debug_struct("WithGracefulShutdown")
            .field("listener", listener)
            .field("make_service", make_service)
            .field("connection_lifetime_limits", connection_lifetime_limits)
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
#[derive(Clone)]
struct HyperExecutor<E>(E);

impl<E, Fut> hyper::rt::Executor<Fut> for HyperExecutor<E>
where
    E: Executor,
    Fut: Future<Output = ()> + Send + 'static,
{
    fn execute(&self, fut: Fut) {
        drop(self.0.execute(fut));
    }
}

async fn handle_connection<L, M, S, B, E>(
    make_service: &mut M,
    signal_rx: &watch::Receiver<()>,
    close_rx: &watch::Receiver<()>,
    io: <L as Listener>::Io,
    remote_addr: <L as Listener>::Addr,
    executor: &E,
    connection_lifetime_limits: ConnectionLifetimeLimits,
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
    let mut signal_rx = signal_rx.clone();
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
        let mut signal_closed = pin!(signal_rx.changed().fuse());

        // Age limit for the connection (with optional jitter). When it
        // elapses we start a graceful shutdown of this connection and re-arm the
        // timer with the grace period (if any), which then bounds how long we
        // wait before forcibly closing.
        let max_age = connection_lifetime_limits.max_connection_age.map(|age| {
            let jitter = random_duration(connection_lifetime_limits.max_connection_age_jitter);
            age.saturating_add(jitter)
        });
        let mut timer = pin!(sleep_or_pending(max_age));
        let mut age_fired = false;

        loop {
            match select(
                conn.as_mut(),
                select(signal_closed.as_mut(), timer.as_mut()),
            )
            .await
            {
                Either::Left((result, _)) => {
                    if let Err(_err) = result {
                        trace!("failed to serve connection: {_err:#}");
                    }
                    break;
                }
                Either::Right((Either::Left((Err(_), _)), _)) => {
                    trace!("signal received in task, starting graceful shutdown");
                    conn.as_mut().graceful_shutdown();
                }
                Either::Right((Either::Left((Ok(()), _)), _)) => {
                    unreachable!("shutdown channel never sends values")
                }
                Either::Right((Either::Right(_), _)) if !age_fired => {
                    age_fired = true;
                    trace!("max connection age reached, starting graceful shutdown");
                    conn.as_mut().graceful_shutdown();
                    timer.set(sleep_or_pending(
                        connection_lifetime_limits.max_connection_age_grace,
                    ));
                }
                Either::Right((Either::Right(_), _)) => {
                    trace!("max connection age grace period elapsed, closing connection");
                    break;
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
/// [`IntoMakeServiceWithConnectInfo`]: crate::extract::connect_info::IntoMakeServiceWithConnectInfo
#[derive(Debug)]
pub struct IncomingStream<'a, L>
where
    L: Listener,
{
    io: &'a TokioIo<L::Io>,
    remote_addr: L::Addr,
}

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
        future::{pending, IntoFuture as _},
        net::{IpAddr, Ipv4Addr},
        pin::pin,
        time::Duration,
    };

    use axum_core::{body::Body, extract::Request};
    use futures_util::future::{select, Either};
    use http::{Response, StatusCode};
    use hyper_util::rt::TokioIo;
    #[cfg(unix)]
    use tokio::net::UnixListener;
    use tokio::{
        io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };
    use tower::ServiceBuilder;

    #[cfg(unix)]
    use super::IncomingStream;
    use super::{serve, ConnectionLifetimeLimits, Listener};
    #[cfg(unix)]
    use crate::extract::connect_info::Connected;
    use crate::{
        body::to_bytes,
        handler::{Handler, HandlerWithoutStateExt},
        routing::get,
        serve::ListenerExt,
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
        serve(tcp_nodelay_listener().await, router.clone()).await;
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

        // with_executor
        let router: Router = Router::new();
        let exec = TestExecutor::new();
        serve(TcpListener::bind(addr).await.unwrap(), router.clone()).with_executor(exec.clone());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .with_executor(exec.clone())
            .with_graceful_shutdown(std::future::pending());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .with_graceful_shutdown(std::future::pending())
            .with_executor(exec.clone());
        serve(TcpListener::bind(addr).await.unwrap(), get(handler)).with_executor(exec.clone());
        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_make_service(),
        )
        .with_executor(exec);

        // connection_lifetime_limits, composable with the other builder methods in any order
        let limits = ConnectionLifetimeLimits::new()
            .max_connection_age(Duration::from_secs(60))
            .max_connection_age_jitter(Duration::from_secs(10))
            .max_connection_age_grace(Duration::from_secs(5));
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .connection_lifetime_limits(limits);
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .connection_lifetime_limits(limits)
            .with_graceful_shutdown(std::future::pending());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .with_graceful_shutdown(std::future::pending())
            .connection_lifetime_limits(limits);
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .connection_lifetime_limits(limits)
            .with_executor(TestExecutor::new());
    }

    async fn handler() {}

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
                serve(ReadyListener(Some(server)), Router::new()).await;
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

            match select(pin!(server_task), pin!(wait_for_server_to_close_conn)).await {
                Either::Left(_) => unreachable!(),
                Either::Right(_) => (),
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

    // Asserts the documented `with_graceful_shutdown` drain semantics: after the
    // signal fires, an already-in-flight request is allowed to run to completion
    // and only then does the `serve` future resolve. The existing
    // `test_with_graceful_shutdown_request_header_timeout` only covers stalled
    // requests being killed by hyper's header read timeout.
    #[crate::test]
    async fn graceful_shutdown_completes_inflight_request() {
        use std::sync::Arc;

        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let app = Router::new().route("/", {
            let started = started.clone();
            let release = release.clone();
            get(move || {
                let started = started.clone();
                let release = release.clone();
                async move {
                    started.notify_one();
                    release.notified().await;
                    "done"
                }
            })
        });

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
        let body = to_bytes(Body::new(response.into_body()), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"done");

        // And only after the in-flight request finished does serve resolve.
        tokio::time::timeout(Duration::from_secs(2), server_task)
            .await
            .expect("serve future did not resolve after in-flight request finished")
            .unwrap();
    }

    // Asserts that `ListenerExt::tap_io` invokes its closure on every accepted
    // connection when used with `serve`. The sibling `ListenerExt::limit_connections`
    // has a direct unit test (in `serve::listener::tests`); `tap_io` did not have
    // a runtime test, so its documented contract was only covered at the type level
    // by `if_it_compiles_it_works`.
    #[crate::test]
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

        let app = Router::new().route("/", get(|| async { "ok" }));
        tokio::spawn(serve(counted, app).into_future());

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

    #[crate::test]
    async fn serving_with_custom_executor() {
        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        let app = Router::new().route("/", get(|| async { "Hello, World!" }));

        let executor = TestExecutor::new();
        tokio::spawn(
            serve(listener, app)
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
        let body = to_bytes(body, usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body, "Hello, World!");

        // One task per connection for HTTP/1.
        assert_eq!(executor.count(), 1);
    }

    #[crate::test]
    #[cfg(feature = "http2")]
    async fn serving_with_custom_executor_http2() {
        use hyper_util::rt::TokioExecutor;

        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        let app = Router::new().route("/", get(|| async { "Hello, World!" }));

        let executor = TestExecutor::new();
        tokio::spawn(
            serve(listener, app)
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
        let body = to_bytes(body, usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body, "Hello, World!");

        // Two tasks: axum's connection, and hyper's internal HTTP/2 task.
        assert_eq!(executor.count(), 2);
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

    #[test]
    fn random_duration_is_bounded_and_varies() {
        use std::collections::HashSet;

        assert_eq!(super::random_duration(Duration::ZERO), Duration::ZERO);

        let max = Duration::from_secs(60);
        let mut seen = HashSet::new();
        for _ in 0..256 {
            let d = super::random_duration(max);
            assert!(d <= max, "{d:?} exceeds the requested bound {max:?}");
            seen.insert(d);
        }

        // It would be astronomically unlikely for 256 draws to all collide if
        // the source is actually random.
        assert!(seen.len() > 1, "random_duration produced a constant value");
    }

    // After `max_connection_age` elapses, an idle keep-alive connection is
    // gracefully shut down by the server, which the client observes as its
    // connection task completing.
    #[tokio::test(start_paused = true)]
    async fn max_connection_age_closes_idle_connection() {
        let app = Router::new().route("/", get(|| async { "ok" }));
        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new().max_connection_age(Duration::from_secs(10)),
                )
                .into_future(),
        );

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        let conn_handle = tokio::spawn(conn);

        // A first request succeeds normally before the age limit elapses.
        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(Body::new(response.into_body()), usize::MAX)
            .await
            .unwrap();

        // With (paused) time auto-advancing, the age timer fires and the server
        // closes the now-idle connection, completing the client's conn task.
        tokio::time::timeout(Duration::from_secs(30), conn_handle)
            .await
            .expect("connection was not closed after max_connection_age elapsed")
            .unwrap()
            .ok();
    }

    // When `max_connection_age` fires while a request is in flight and the
    // handler never completes, the grace period bounds how long the server
    // waits before forcibly closing, so the in-flight request fails.
    #[tokio::test(start_paused = true)]
    async fn max_connection_age_grace_force_closes_stuck_connection() {
        use std::{future::pending, sync::Arc};

        use tokio::sync::Notify;

        let started = Arc::new(Notify::new());
        let app = Router::new().route("/", {
            let started = started.clone();
            get(move || {
                let started = started.clone();
                async move {
                    started.notify_one();
                    pending::<()>().await;
                    "unreachable"
                }
            })
        });

        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new()
                        .max_connection_age(Duration::from_secs(10))
                        .max_connection_age_grace(Duration::from_secs(5)),
                )
                .into_future(),
        );

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let send = tokio::spawn(async move { sender.send_request(request).await });

        // Wait until the (never-completing) handler is actually running.
        started.notified().await;

        // age (10s) + grace (5s) later, the connection is force-closed despite
        // the stuck handler, so the in-flight request resolves with an error.
        let result = tokio::time::timeout(Duration::from_secs(60), send)
            .await
            .expect("request was not aborted within the grace period")
            .unwrap();
        assert!(
            result.is_err(),
            "expected the in-flight request to fail when the connection is force-closed",
        );
    }

    // Without a grace period, `max_connection_age` only stops new requests: one
    // that is still in flight when the age limit fires keeps the connection
    // alive for as long as it needs and still completes successfully. Only
    // `max_connection_age_grace` opts into force-closing in-flight work.
    #[tokio::test(start_paused = true)]
    async fn max_connection_age_without_grace_lets_inflight_request_finish() {
        use std::sync::Arc;

        use tokio::sync::Notify;

        let started = Arc::new(Notify::new());
        let released = Arc::new(Notify::new());

        let app = Router::new().route("/", {
            let started = started.clone();
            let released = released.clone();
            get(move || {
                let started = started.clone();
                let released = released.clone();
                async move {
                    started.notify_one();
                    released.notified().await;
                    "done"
                }
            })
        });

        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new().max_connection_age(Duration::from_secs(10)),
                )
                .into_future(),
        );

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let send = tokio::spawn(async move { sender.send_request(request).await });

        // Wait until the handler is actually running, then advance (paused)
        // time well past the age limit while the request is still in flight.
        started.notified().await;
        tokio::time::sleep(Duration::from_secs(60)).await;

        // Release the handler; the response must still arrive because the age
        // limit alone never cuts in-flight requests.
        released.notify_one();
        let response = tokio::time::timeout(Duration::from_secs(5), send)
            .await
            .expect("in-flight request did not resolve after the age limit fired")
            .unwrap()
            .expect("in-flight request failed: the age limit must not cut in-flight requests");
        assert_eq!(response.status(), StatusCode::OK);
    }

    // The HTTP/2 equivalent of `max_connection_age_closes_idle_connection`: the
    // server sends GOAWAY once the age limit elapses, completing the client's
    // connection task.
    #[cfg(feature = "http2")]
    #[tokio::test(start_paused = true)]
    async fn max_connection_age_closes_idle_connection_http2() {
        use hyper_util::rt::TokioExecutor;

        let app = Router::new().route("/", get(|| async { "ok" }));
        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new().max_connection_age(Duration::from_secs(10)),
                )
                .into_future(),
        );

        let io = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http2::Builder::new(TokioExecutor::new())
            .handshake(io)
            .await
            .unwrap();
        let conn_handle = tokio::spawn(conn);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(Body::new(response.into_body()), usize::MAX)
            .await
            .unwrap();

        // GOAWAY after the age limit closes the connection from the server side.
        tokio::time::timeout(Duration::from_secs(30), conn_handle)
            .await
            .expect("HTTP/2 connection was not closed after max_connection_age elapsed")
            .unwrap()
            .ok();
    }
}
