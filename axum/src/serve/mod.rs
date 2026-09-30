//! Serve services.

use std::{
    convert::Infallible,
    error::Error as StdError,
    fmt::Debug,
    future::{Future, IntoFuture, Pending},
    hash::{BuildHasher, Hasher},
    io,
    marker::PhantomData,
    pin::{pin, Pin},
    sync::Arc,
    task::{ready, Context, Poll},
    time::Duration,
};

use axum_core::{body::Body, extract::Request, response::Response};
use futures_core::future::BoxFuture;
use futures_util::{
    future::{select, Either},
    FutureExt,
};
use http::{header, HeaderValue, Method, StatusCode, Version};
use http_body::Body as HttpBody;
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
#[cfg(feature = "http1")]
use hyper_util::rt::TokioTimer;
#[cfg(any(feature = "http1", feature = "http2"))]
use hyper_util::{server::conn::auto::Builder, service::TowerToHyperService};
use pin_project_lite::pin_project;
use tokio::{
    sync::{watch, Notify},
    task::JoinHandle,
};
use tower::ServiceExt as _;
use tower_service::Service;

mod listener;

pub use self::listener::{ConnLimiter, ConnLimiterIo, Listener, ListenerExt, TapIo};

/// Serve the service with the supplied listener.
///
/// This method of running a service exposes only some configuration knobs: how connection
/// tasks are spawned (via [`Serve::with_executor`]) and how long connections live (via
/// [`Serve::connection_lifetime_limits`]). Everything else uses hyper's default configuration
/// (including [timeouts]); use hyper or hyper-util if you need more control.
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
        rotation_signal: None,
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
/// The only limit currently available is [`MaxConnectionAge`].
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use axum::{Router, routing::get, serve::{ConnectionLifetimeLimits, MaxConnectionAge}};
///
/// # async {
/// let router = Router::new().route("/", get(|| async { "Hello, World!" }));
/// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
///
/// let limits = ConnectionLifetimeLimits::new().max_connection_age(
///     MaxConnectionAge::new(Duration::from_secs(10 * 60))
///         // Random per-connection jitter added to the age, to avoid
///         // synchronized reconnect storms when many connections were
///         // established at once.
///         .jitter(Duration::from_secs(60))
///         // Hard cap on how long to wait for in-flight work after the age
///         // limit fires before forcibly closing.
///         .grace(Duration::from_secs(30)),
/// );
///
/// axum::serve(listener, router)
///     .connection_lifetime_limits(limits)
///     .await;
/// # };
/// ```
#[derive(Clone, Debug, Default)]
#[must_use]
pub struct ConnectionLifetimeLimits {
    max_connection_age: Option<MaxConnectionAge>,
}

impl ConnectionLifetimeLimits {
    /// Create a new [`ConnectionLifetimeLimits`] with no limits set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cap how long a connection keeps accepting new requests.
    ///
    /// See [`MaxConnectionAge`] for what the limit does and how to configure
    /// its jitter and grace period.
    ///
    /// Connection age is unbounded by default.
    ///
    /// # Upgraded connections
    ///
    /// This limit does not apply to connections that have been upgraded, e.g.
    /// to a WebSocket or via `CONNECT`. Once an HTTP/1 connection is upgraded,
    /// hyper hands the socket off to the upgrade handler and [`serve`] stops
    /// tracking it, so neither the age limit nor the grace period affect it.
    /// An HTTP/2 upgrade stays a stream on the tracked connection and is
    /// treated like any other in-flight stream.
    pub fn max_connection_age(mut self, age: MaxConnectionAge) -> Self {
        self.max_connection_age = Some(age);
        self
    }
}

/// A cap on how long a connection keeps accepting new requests, applied by
/// [`serve`] via [`ConnectionLifetimeLimits::max_connection_age`].
///
/// Once a connection has been open for [`new`]'s duration (plus [`jitter`]), a
/// graceful shutdown of that connection is started. The clock starts when the
/// connection task is spawned, which happens after the connection has been
/// accepted and the service for it has been created, rather than at accept
/// time. The mechanism differs by protocol:
///
/// - **HTTP/1**: the next response gets a `Connection: close` header and the
///   connection is closed once the in-flight request finishes.
/// - **HTTP/2**: a `GOAWAY` is sent, so new streams are refused while in-flight
///   streams are allowed to finish.
///
/// In both cases in-flight work is waited on for as long as it takes, unless
/// [`grace`] is set: once the grace period elapses the connection is closed even
/// if a request is still in flight. See [`grace`] for the trade-off.
///
/// [`new`]: MaxConnectionAge::new
/// [`jitter`]: MaxConnectionAge::jitter
/// [`grace`]: MaxConnectionAge::grace
#[derive(Clone, Debug)]
#[must_use]
pub struct MaxConnectionAge {
    age: Duration,
    jitter: Duration,
    grace: Option<Duration>,
}

impl MaxConnectionAge {
    /// Create a new `MaxConnectionAge` that stops a connection from accepting
    /// new requests once it has been open for `age`.
    ///
    /// Consider also setting [`jitter`] to avoid all connections opened around
    /// the same time tearing down simultaneously.
    ///
    /// [`jitter`]: MaxConnectionAge::jitter
    pub fn new(age: Duration) -> Self {
        Self {
            age,
            jitter: Duration::ZERO,
            grace: None,
        }
    }

    /// Set the maximum random jitter added to the age.
    ///
    /// Each connection adds a random duration in `[0, jitter]` to its age limit.
    /// This is important for avoiding synchronized reconnect storms when many
    /// connections were established at the same time (e.g. right after a
    /// deploy): without it, every connection opened in the same instant tears
    /// down in the same instant once the age limit elapses.
    ///
    /// Defaults to `Duration::ZERO`, i.e. no jitter.
    pub fn jitter(mut self, jitter: Duration) -> Self {
        self.jitter = jitter;
        self
    }

    /// Set a hard cap on how long to wait for in-flight work after the age
    /// limit fires before forcibly closing the connection.
    ///
    /// Without a grace period the age limit only stops new requests: the server
    /// waits however long it takes for in-flight work to finish before closing
    /// the connection. Setting a grace period turns age (+ jitter) + grace into
    /// a hard deadline: when it elapses the connection is closed *even if a
    /// request is still in flight*, and the client never receives a response for
    /// it. This applies to HTTP/1 requests as well as HTTP/2 streams, so a
    /// handler that runs longer than the age limit plus the grace period will
    /// never complete successfully. Only set a grace period if bounding
    /// connection lifetime matters more than letting slow requests finish.
    ///
    /// `None` waits for in-flight work for as long as it takes, which is the
    /// default.
    pub fn grace(mut self, grace: impl Into<Option<Duration>>) -> Self {
        self.grace = grace.into();
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
    rotation_signal: Option<BoxFuture<'static, ()>>,
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
    ///
    /// To move clients to another server before you shut this one down, see
    /// [`with_connection_rotation`](Self::with_connection_rotation).
    pub fn with_graceful_shutdown<F>(self, signal: F) -> WithGracefulShutdown<L, M, S, F, B, E>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        WithGracefulShutdown {
            listener: self.listener,
            make_service: self.make_service,
            executor: self.executor,
            connection_lifetime_limits: self.connection_lifetime_limits,
            rotation_signal: self.rotation_signal,
            signal,
            _marker: PhantomData,
        }
    }

    /// Starts to close connections when the provided future completes, and keeps serving.
    ///
    /// This is also known as "lame duck" mode. The server still accepts connections and still
    /// answers requests. It only tells every client that the connection it uses must not be used
    /// again. Clients then open a new connection, and a load balancer can send them to another
    /// server. Use this before a shutdown, while this server can still answer requests.
    ///
    /// What the client sees depends on the protocol:
    ///
    /// * HTTP/1: the response to the request that runs at that moment has a `connection: close`
    ///   header. The connection closes after that response. A connection that is idle at that
    ///   moment closes immediately, as it does after an idle timeout.
    /// * HTTP/2: the server sends a GOAWAY frame. Requests that run at that moment finish. The
    ///   client starts no new request on that connection.
    ///
    /// Connections that are accepted after the signal are served in the same way: each one answers
    /// its request, and that response tells the client to close the connection. A connection that
    /// has not sent a request yet stays open until it has been answered once. The server never
    /// closes a connection before it has answered the request on it.
    ///
    /// A connection that was upgraded, for example a WebSocket connection, is not closed. Such a
    /// connection is no longer an HTTP connection that the server can shut down.
    ///
    /// # Cost
    ///
    /// In this mode each HTTP/1 connection serves about one request. A client that keeps sending
    /// requests to this server must open a new connection for almost every request. This costs
    /// time on both sides. Use this mode only for the short time before a shutdown. Do not use it
    /// as a permanent setting.
    ///
    /// # Difference to graceful shutdown
    ///
    /// [`with_graceful_shutdown`](Self::with_graceful_shutdown) stops the server. It stops
    /// accepting connections, lets the requests that run at that moment finish, and then the
    /// future returned by [`serve`] completes.
    ///
    /// This method does not stop the server. The server keeps accepting connections and keeps
    /// answering requests. Use both methods together to first move the clients away, and then to
    /// stop.
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
    ///     .with_connection_rotation(rotation_signal())
    ///     .with_graceful_shutdown(shutdown_signal())
    ///     .await;
    /// # };
    ///
    /// async fn rotation_signal() {
    ///     // for example, wait for `SIGTERM`
    /// }
    ///
    /// async fn shutdown_signal() {
    ///     // for example, wait a few seconds after `SIGTERM`
    /// }
    /// ```
    ///
    /// The signal is a future, so any channel can start the rotation. This sends the signal from
    /// another task:
    ///
    /// ```
    /// use axum::{Router, routing::get};
    ///
    /// # async {
    /// let router = Router::new().route("/", get(|| async { "Hello, World!" }));
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    ///
    /// let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    ///
    /// tokio::spawn(async move {
    ///     // ...
    ///     let _ = tx.send(());
    /// });
    ///
    /// axum::serve(listener, router)
    ///     .with_connection_rotation(async move {
    ///         let _ = rx.await;
    ///     })
    ///     .await;
    /// # };
    /// ```
    ///
    /// This method can be called before or after
    /// [`with_graceful_shutdown`](Self::with_graceful_shutdown).
    pub fn with_connection_rotation<F>(mut self, signal: F) -> Self
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.rotation_signal = Some(Box::pin(signal));
        self
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
            rotation_signal: self.rotation_signal,
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
            rotation_signal,
            _marker,
        } = self;

        let (_signal_tx, signal_rx) = watch::channel(());
        let (_close_tx, close_rx) = watch::channel(());
        let rotation_rx = rotation_channel(rotation_signal, &executor);
        let signals = ConnectionSignals {
            shutdown: signal_rx,
            rotation: rotation_rx,
            close: close_rx,
        };

        loop {
            let (io, remote_addr) = listener.accept().await;
            handle_connection(
                &mut make_service,
                &signals,
                io,
                remote_addr,
                &executor,
                &connection_lifetime_limits,
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
            rotation_signal,
            _marker: _,
        } = self;

        let mut s = f.debug_struct("Serve");
        s.field("listener", listener)
            .field("make_service", make_service)
            .field("executor", executor)
            .field("connection_lifetime_limits", connection_lifetime_limits)
            .field("connection_rotation", &rotation_signal.is_some());

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
    rotation_signal: Option<BoxFuture<'static, ()>>,
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
            rotation_signal: self.rotation_signal,
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

    /// Starts to close connections when the provided future completes, and keeps serving.
    ///
    /// See [`Serve::with_connection_rotation`] for details.
    pub fn with_connection_rotation<F2>(mut self, signal: F2) -> Self
    where
        F2: Future<Output = ()> + Send + 'static,
    {
        self.rotation_signal = Some(Box::pin(signal));
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
            rotation_signal,
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
        let rotation_rx = rotation_channel(rotation_signal, &executor);
        let signals = ConnectionSignals {
            shutdown: signal_rx.clone(),
            rotation: rotation_rx,
            close: close_rx,
        };

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
                &signals,
                io,
                remote_addr,
                &executor,
                &connection_lifetime_limits,
            )
            .await;
        }

        // Drops this runner's own `close` receiver along with the rest, so that
        // `close_tx` is left with only the connection tasks' receivers.
        drop(signals);
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
            rotation_signal,
            signal,
            _marker: _,
        } = self;

        f.debug_struct("WithGracefulShutdown")
            .field("listener", listener)
            .field("make_service", make_service)
            .field("connection_lifetime_limits", connection_lifetime_limits)
            .field("signal", signal)
            .field("connection_rotation", &rotation_signal.is_some())
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

/// Creates the channel that tells the connection tasks to close their connections.
///
/// The sender is dropped once `signal` completes. Without a signal the sender is given back to the
/// caller, which keeps it open for as long as the server runs.
/// The watch channels a connection task listens on, bundled so that each one
/// does not have to be threaded through [`handle_connection`] separately.
struct ConnectionSignals {
    /// Graceful shutdown. Its sender is dropped once the server stops
    /// accepting, and the connection task then shuts its connection down and
    /// waits for the requests still in flight.
    shutdown: watch::Receiver<()>,
    /// Connection rotation, or `None` when no rotation signal was given. Its
    /// sender is dropped once connections start rotating, and the connection
    /// task then shuts its connection down while the server keeps accepting.
    rotation: Option<watch::Receiver<()>>,
    /// Dropped by the connection task when it is done, so that the server can
    /// wait for every task before resolving.
    close: watch::Receiver<()>,
}

fn rotation_channel<E>(
    signal: Option<BoxFuture<'static, ()>>,
    executor: &E,
) -> Option<watch::Receiver<()>>
where
    E: Executor,
{
    let signal = signal?;

    let (rotation_tx, rotation_rx) = watch::channel(());

    executor.execute(async move {
        // The signal is the caller's future and may never complete, so stop
        // waiting on it once every receiver is gone. Otherwise this task, and
        // the signal it holds, would outlive the server it rotates.
        let closed = rotation_tx.closed();
        if let Either::Right(_) = select(signal, pin!(closed)).await {
            trace!("server stopped before the connection rotation signal, dropping it");
            return;
        }
        trace!("received connection rotation signal. Telling tasks to close their connections");
        drop(rotation_tx);
    });

    Some(rotation_rx)
}

async fn handle_connection<L, M, S, B, E>(
    make_service: &mut M,
    signals: &ConnectionSignals,
    io: <L as Listener>::Io,
    remote_addr: <L as Listener>::Addr,
    executor: &E,
    connection_lifetime_limits: &ConnectionLifetimeLimits,
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
    let mut signal_rx = signals.shutdown.clone();
    let connection_lifetime_limits = connection_lifetime_limits.clone();
    let io = TokioIo::new(io);

    trace!("connection {remote_addr:?} accepted");

    // Nothing to watch and nothing to notify when no rotation signal was given.
    let rotation = signals
        .rotation
        .clone()
        .map(|rotation_rx| (rotation_rx, Arc::new(Notify::new())));
    // Notified once the connection has a request to answer.
    let first_request = rotation.as_ref().map(|(_, notify)| notify.clone());
    let rotation_rx = rotation
        .as_ref()
        .map(|(rotation_rx, _)| rotation_rx.clone());

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
        .map_request(move |req: Request<Incoming>| {
            if let Some(first_request) = &first_request {
                first_request.notify_one();
            }
            req.map(Body::new)
        });

    let hyper_service = CloseWhenRotating {
        inner: TowerToHyperService::new(tower_service),
        rotation: rotation_rx,
    };
    let close_rx = signals.close.clone();

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

        // Neither channel sends values, so `changed` only returns once its
        // sender is dropped.
        let signal_closed = async {
            let _ = signal_rx.changed().await;
            trace!("signal received in task, starting graceful shutdown");
        };

        let rotation_started = async {
            let Some((mut rotation_rx, first_request)) = rotation else {
                return std::future::pending().await;
            };
            let _ = rotation_rx.changed().await;
            // hyper closes a connection that has not read a request yet, instead of shutting it
            // down gracefully. Waiting for the first request keeps that exchange alive: the client
            // gets its response, and that response is what tells it to close the connection.
            first_request.notified().await;
            trace!("rotation signal received in task, starting graceful shutdown");
        };

        // Both signals start the same graceful shutdown of this connection, so
        // only the first one matters. Fused because it fires at most once.
        let mut shutdown = pin!(async {
            select(pin!(signal_closed), pin!(rotation_started)).await;
        }
        .fuse());

        // Age limit for the connection (with optional jitter). When it
        // elapses we start a graceful shutdown of this connection and re-arm the
        // timer with the grace period (if any), which then bounds how long we
        // wait before forcibly closing.
        let max_connection_age = connection_lifetime_limits.max_connection_age;
        let max_age = max_connection_age
            .as_ref()
            .map(|limit| limit.age.saturating_add(random_duration(limit.jitter)));
        let grace = max_connection_age.and_then(|limit| limit.grace);
        let mut timer = pin!(sleep_or_pending(max_age));
        let mut age_fired = false;

        loop {
            match select(conn.as_mut(), select(shutdown.as_mut(), timer.as_mut())).await {
                Either::Left((result, _)) => {
                    if let Err(_err) = result {
                        trace!("failed to serve connection: {_err:#}");
                    }
                    break;
                }
                Either::Right((Either::Left(((), _)), _)) => {
                    conn.as_mut().graceful_shutdown();
                }
                Either::Right((Either::Right(_), _)) if !age_fired => {
                    age_fired = true;
                    trace!("max connection age reached, starting graceful shutdown");
                    conn.as_mut().graceful_shutdown();
                    timer.set(sleep_or_pending(grace));
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

/// Tells an HTTP/1 client that its connection is over, by putting
/// `connection: close` on the response to the request it has just sent.
///
/// The connection task starts a graceful shutdown as well, but it can only do
/// that the next time it polls the connection, and a handler that answers
/// straight away leaves hyper nothing to wait for: the response goes out inside
/// the same poll, before the task runs again. The client would then be given a
/// response that says the connection is reusable, and the connection closed
/// under it. Setting the header where the response is produced puts it on the
/// response the client actually receives, whenever the handler finishes.
///
/// HTTP/2 has no `connection` header, and does not need one: a `GOAWAY` reaches
/// the client whenever the connection task sends it, without having to ride on
/// a response.
struct CloseWhenRotating<S> {
    inner: S,
    /// The rotation channel, or `None` when no rotation signal was given. Its
    /// sender is dropped once connections start rotating.
    rotation: Option<watch::Receiver<()>>,
}

impl<S, B> hyper::service::Service<Request<Incoming>> for CloseWhenRotating<S>
where
    S: hyper::service::Service<Request<Incoming>, Response = Response<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = CloseWhenRotatingFuture<S::Future>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        // HTTP/2 has no `connection` header, and a `CONNECT` that succeeds
        // turns the connection into a tunnel rather than ending it, so neither
        // response is one to put the header on. Whether connections are
        // rotating is asked later, once the response is ready: rotation may
        // start while this request is still being handled.
        let rotation = (req.version() < Version::HTTP_2 && req.method() != Method::CONNECT)
            .then(|| self.rotation.clone())
            .flatten();

        CloseWhenRotatingFuture {
            rotation,
            inner: self.inner.call(req),
        }
    }
}

pin_project! {
    /// Response future for [`CloseWhenRotating`].
    struct CloseWhenRotatingFuture<F> {
        #[pin]
        inner: F,
        // The rotation channel, or `None` when this response is not one to put
        // the header on.
        rotation: Option<watch::Receiver<()>>,
    }
}

impl<F, B, E> Future for CloseWhenRotatingFuture<F>
where
    F: Future<Output = Result<Response<B>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let mut response = ready!(this.inner.poll(cx)?);

        // `has_changed` fails once the sender has been dropped, which is what
        // starting the rotation does. Asking here rather than when the request
        // arrived is what covers a rotation that started while the handler ran,
        // which is otherwise the one case the header would miss.
        let rotating = this
            .rotation
            .as_ref()
            .is_some_and(|rotation| rotation.has_changed().is_err());

        // An upgrade answers with `connection: upgrade`, and the connection
        // carries on as something that is no longer HTTP for us to close.
        if rotating && response.status() != StatusCode::SWITCHING_PROTOCOLS {
            response
                .headers_mut()
                .append(header::CONNECTION, HeaderValue::from_static("close"));
        }

        Poll::Ready(Ok(response))
    }
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
    use super::{serve, ConnectionLifetimeLimits, Listener, MaxConnectionAge};
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

    #[allow(dead_code, unused)]
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

        // with_connection_rotation, in both orders
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .with_connection_rotation(std::future::pending())
            .with_graceful_shutdown(std::future::pending())
            .with_executor(exec.clone());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .with_graceful_shutdown(std::future::pending())
            .with_connection_rotation(std::future::pending())
            .with_executor(exec.clone());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .with_connection_rotation(std::future::pending())
            .with_executor(exec.clone());

        serve(TcpListener::bind(addr).await.unwrap(), get(handler)).with_executor(exec.clone());
        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_make_service(),
        )
        .with_executor(exec);

        // connection_lifetime_limits, composable with the other builder methods in any order
        let limits = ConnectionLifetimeLimits::new().max_connection_age(
            MaxConnectionAge::new(Duration::from_secs(60))
                .jitter(Duration::from_secs(10))
                .grace(Duration::from_secs(5)),
        );
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .connection_lifetime_limits(limits.clone());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .connection_lifetime_limits(limits.clone())
            .with_graceful_shutdown(std::future::pending());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .with_graceful_shutdown(std::future::pending())
            .connection_lifetime_limits(limits.clone());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .connection_lifetime_limits(limits.clone())
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

    const GET_ROOT: &[u8] = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";

    // Sends one request and then reads until the server closes the connection.
    async fn request_until_close<S>(stream: &mut S) -> String
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        stream.write_all(GET_ROOT).await.unwrap();
        read_until_close(stream).await
    }

    // Reads until the server closes the connection.
    async fn read_until_close<S>(stream: &mut S) -> String
    where
        S: AsyncRead + Unpin,
    {
        let mut response = String::new();
        tokio::time::timeout(Duration::from_secs(2), stream.read_to_string(&mut response))
            .await
            .expect("the server did not close the connection")
            .unwrap();
        response
    }

    fn assert_closing_response(response: &str, body: &str) {
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response:?}");
        assert!(
            response.to_lowercase().contains("connection: close"),
            "{response:?}"
        );
        assert!(response.ends_with(body), "{response:?}");
    }

    // A request that is in flight when the rotation signal fires is answered, and its response
    // tells the client that the connection is over.
    #[crate::test]
    async fn connection_rotation_closes_an_existing_connection() {
        use std::sync::Arc;

        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let (rotate_tx, rotate_rx) = tokio::sync::oneshot::channel::<()>();

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

        let (mut client, server) = io::duplex(1024);

        tokio::spawn(
            serve(ReadyListener(Some(server)), app)
                .with_connection_rotation(async move {
                    rotate_rx.await.ok();
                })
                .into_future(),
        );

        client.write_all(GET_ROOT).await.unwrap();

        // Wait until the handler runs, so that the request is in flight.
        started.notified().await;
        rotate_tx.send(()).unwrap();

        // Give the connection task time to see the signal before the response is written.
        tokio::time::sleep(Duration::from_millis(50)).await;
        release.notify_one();

        let response = read_until_close(&mut client).await;

        assert_closing_response(&response, "done");
    }

    // A connection that is accepted while connections rotate is served as well. Without this, a
    // client that reconnects during the rotation would sit on this server again.
    #[crate::test]
    async fn connection_rotation_serves_new_connections() {
        let (rotate_tx, rotate_rx) = tokio::sync::oneshot::channel::<()>();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let app = Router::new().route("/", get(|| async { "hello" }));

        tokio::spawn(
            serve(listener, app)
                .with_connection_rotation(async move {
                    rotate_rx.await.ok();
                })
                .into_future(),
        );

        rotate_tx.send(()).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // The server keeps accepting. Each connection answers its request and closes after it.
        for _ in 0..2 {
            let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            let response = request_until_close(&mut stream).await;

            assert_closing_response(&response, "hello");
        }
    }

    // A connection that has not sent a request yet is still deciding which protocol it speaks.
    // Shutting it down at that point would close it before the client is served.
    #[crate::test]
    async fn connection_rotation_waits_for_the_first_request() {
        let (rotate_tx, rotate_rx) = tokio::sync::oneshot::channel::<()>();

        let app = Router::new().route("/", get(|| async { "hello" }));

        let (mut client, server) = io::duplex(1024);

        tokio::spawn(
            serve(ReadyListener(Some(server)), app)
                .with_connection_rotation(async move {
                    rotate_rx.await.ok();
                })
                .into_future(),
        );

        // Give the server time to accept the connection. The client has sent nothing on it, so the
        // server does not know yet which protocol the client speaks.
        tokio::time::sleep(Duration::from_millis(50)).await;

        rotate_tx.send(()).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // The request is answered, that response tells the client that the connection is over,
        // and the connection closes after it.
        let response = request_until_close(&mut client).await;

        assert_closing_response(&response, "hello");
    }

    // A connection that is upgraded during rotation is no longer an HTTP connection that the
    // server can close, so its response must not claim otherwise.
    #[cfg(all(feature = "http1", feature = "ws"))]
    #[crate::test]
    async fn connection_rotation_leaves_an_upgrade_alone() {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::{tungstenite, WebSocketStream};

        use crate::{
            extract::ws::{Message, WebSocketUpgrade},
            routing::any,
        };

        let (rotate_tx, rotate_rx) = tokio::sync::oneshot::channel::<()>();

        let app = Router::new().route(
            "/ws",
            any(|ws: WebSocketUpgrade| async move {
                ws.on_upgrade(|mut socket| async move {
                    while let Some(Ok(Message::Text(text))) = socket.recv().await {
                        socket.send(Message::Text(text)).await.ok();
                    }
                })
            }),
        );

        let (client, server) = io::duplex(1024);

        tokio::spawn(
            serve(ReadyListener(Some(server)), app)
                .with_connection_rotation(async move {
                    rotate_rx.await.ok();
                })
                .into_future(),
        );

        tokio::time::sleep(Duration::from_millis(50)).await;
        rotate_tx.send(()).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn.with_upgrades());

        let request = Request::builder()
            .uri("/ws")
            .header("connection", "upgrade")
            .header("upgrade", "websocket")
            .header("sec-websocket-version", "13")
            .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
            .body(Body::empty())
            .unwrap();
        let mut response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
        let closing = response
            .headers()
            .get_all(http::header::CONNECTION)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .any(|token| token.trim().eq_ignore_ascii_case("close"));
        assert!(
            !closing,
            "the upgrade response told the client to close the connection: {:?}",
            response.headers(),
        );

        let upgraded = hyper::upgrade::on(&mut response).await.unwrap();
        let mut ws = WebSocketStream::from_raw_socket(
            TokioIo::new(upgraded),
            tungstenite::protocol::Role::Client,
            None,
        )
        .await;

        ws.send(tungstenite::Message::Text("hi".into()))
            .await
            .expect("the upgraded connection was closed by the rotation");
        let echoed = ws
            .next()
            .await
            .expect("the upgraded connection ended")
            .expect("the upgraded connection failed");
        assert_eq!(echoed.into_text().unwrap().as_str(), "hi");
    }

    // The rotation signal is the caller's future and may never complete. The task that waits on it
    // must not outlive the server, or it keeps whatever that future holds alive with it.
    #[crate::test]
    async fn connection_rotation_signal_does_not_outlive_the_server() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };

        struct SetOnDrop(Arc<AtomicBool>);

        impl Drop for SetOnDrop {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let dropped = Arc::new(AtomicBool::new(false));
        let guard = SetOnDrop(dropped.clone());

        let (_client, server) = io::duplex(1024);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let server = tokio::spawn(
            serve(ReadyListener(Some(server)), Router::new())
                // A rotation signal that never completes, holding a resource.
                .with_connection_rotation(async move {
                    let _guard = guard;
                    std::future::pending::<()>().await;
                })
                .with_graceful_shutdown(async move {
                    shutdown_rx.await.ok();
                })
                .into_future(),
        );

        shutdown_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("the server did not stop")
            .unwrap();

        for _ in 0..50 {
            tokio::task::yield_now().await;
        }

        assert!(
            dropped.load(Ordering::SeqCst),
            "the rotation signal outlived the server",
        );
    }

    // HTTP/2 clients get a GOAWAY frame. The request that runs at that moment still finishes, and
    // the connection ends after it.
    #[crate::test]
    #[cfg(feature = "http2")]
    async fn connection_rotation_closes_an_existing_http2_connection() {
        use std::sync::Arc;

        use hyper_util::rt::TokioExecutor;

        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let (rotate_tx, rotate_rx) = tokio::sync::oneshot::channel::<()>();

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

        tokio::spawn(
            serve(ReadyListener(Some(server)), app)
                .with_connection_rotation(async move {
                    rotate_rx.await.ok();
                })
                .into_future(),
        );

        let io = TokioIo::new(client);
        let (sender, conn) = hyper::client::conn::http2::Builder::new(TokioExecutor::new())
            .handshake(io)
            .await
            .unwrap();
        let conn_handle = tokio::spawn(conn);

        // This clone stays alive for the whole test. An HTTP/2 client connection only ends on its
        // own once every sender is dropped, so the connection can only end here because the server
        // closed it.
        let mut idle_sender = sender.clone();

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let request_fut = tokio::spawn({
            let mut sender = sender;
            async move { sender.send_request(request).await }
        });

        // Wait until the handler runs, so that the request is in flight.
        started.notified().await;
        rotate_tx.send(()).unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        release.notify_one();

        let response = request_fut.await.unwrap().unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(Body::new(response.into_body()), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"done");

        // The client's connection ends once the server has sent the GOAWAY frame and the request
        // has finished.
        tokio::time::timeout(Duration::from_secs(2), conn_handle)
            .await
            .expect("connection did not end after the rotation signal")
            .unwrap()
            .unwrap();

        // The client cannot start a new request on it either.
        assert!(idle_sender.ready().await.is_err());
    }

    // Rotation keeps the server running. Only the graceful shutdown signal stops it.
    #[crate::test]
    async fn connection_rotation_then_graceful_shutdown() {
        let (rotate_tx, rotate_rx) = tokio::sync::oneshot::channel::<()>();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let app = Router::new().route("/", get(|| async { "hello" }));

        let server_task = tokio::spawn(
            serve(listener, app)
                .with_connection_rotation(async move {
                    rotate_rx.await.ok();
                })
                .with_graceful_shutdown(async move {
                    shutdown_rx.await.ok();
                })
                .into_future(),
        );

        rotate_tx.send(()).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // The server still answers requests while it rotates connections.
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let response = request_until_close(&mut stream).await;
        assert_closing_response(&response, "hello");

        assert!(
            !server_task.is_finished(),
            "serve resolved before the graceful shutdown signal",
        );

        shutdown_tx.send(()).unwrap();

        tokio::time::timeout(Duration::from_secs(2), server_task)
            .await
            .expect("serve future did not resolve after the graceful shutdown signal")
            .unwrap();
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
                    ConnectionLifetimeLimits::new()
                        .max_connection_age(MaxConnectionAge::new(Duration::from_secs(10))),
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
                .connection_lifetime_limits(ConnectionLifetimeLimits::new().max_connection_age(
                    MaxConnectionAge::new(Duration::from_secs(10)).grace(Duration::from_secs(5)),
                ))
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
                    ConnectionLifetimeLimits::new()
                        .max_connection_age(MaxConnectionAge::new(Duration::from_secs(10))),
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
                    ConnectionLifetimeLimits::new()
                        .max_connection_age(MaxConnectionAge::new(Duration::from_secs(10))),
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

    // The HTTP/2 equivalent of
    // `max_connection_age_grace_force_closes_stuck_connection`: once the grace
    // period elapses the connection is closed even though an HTTP/2 stream is
    // still open, so the in-flight request fails.
    #[cfg(feature = "http2")]
    #[tokio::test(start_paused = true)]
    async fn max_connection_age_grace_force_closes_stuck_connection_http2() {
        use std::{future::pending, sync::Arc};

        use hyper_util::rt::TokioExecutor;
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
                .connection_lifetime_limits(ConnectionLifetimeLimits::new().max_connection_age(
                    MaxConnectionAge::new(Duration::from_secs(10)).grace(Duration::from_secs(5)),
                ))
                .into_future(),
        );

        let io = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http2::Builder::new(TokioExecutor::new())
            .handshake(io)
            .await
            .unwrap();
        tokio::spawn(conn);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let send = tokio::spawn(async move { sender.send_request(request).await });

        // Wait until the (never-completing) handler is actually running.
        started.notified().await;

        // age (10s) + grace (5s) later, the connection is force-closed despite
        // the still-open stream, so the in-flight request resolves with an error.
        let result = tokio::time::timeout(Duration::from_secs(60), send)
            .await
            .expect("request was not aborted within the grace period")
            .unwrap();
        assert!(
            result.is_err(),
            "expected the in-flight HTTP/2 request to fail when the connection is force-closed",
        );
    }

    // The grace period is an upper bound, not a deadline that requests are cut
    // at: a request that finishes after the age limit fires but before the
    // grace period elapses still gets its response.
    #[tokio::test(start_paused = true)]
    async fn max_connection_age_grace_lets_request_finish_in_time() {
        // The age limit fires at 10s and the grace period force-closes at 15s,
        // so a handler that takes 12s completes in between the two.
        let app = Router::new().route(
            "/",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(12)).await;
                "done"
            }),
        );

        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(ConnectionLifetimeLimits::new().max_connection_age(
                    MaxConnectionAge::new(Duration::from_secs(10)).grace(Duration::from_secs(5)),
                ))
                .into_future(),
        );

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = tokio::time::timeout(Duration::from_secs(60), sender.send_request(request))
            .await
            .expect("in-flight request did not resolve")
            .expect("in-flight request failed even though it finished within the grace period");
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(Body::new(response.into_body()), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"done");
    }

    // Jitter is added to the age limit of each individual connection, so
    // connections opened at the same instant close at different times spread
    // over `[age, age + jitter]` rather than all closing exactly at `age`.
    #[tokio::test(start_paused = true)]
    async fn max_connection_age_jitter_extends_deadline() {
        const CONNECTIONS: usize = 8;

        let age = Duration::from_secs(1);
        // Kept comfortably below hyper's 30s header read timeout, which would
        // otherwise close these never-used connections before their deadline.
        let jitter = Duration::from_secs(20);

        let start = tokio::time::Instant::now();

        // The senders are held for the duration of the test: dropping one would
        // close its connection from the client side.
        let mut senders: Vec<hyper::client::conn::http1::SendRequest<Body>> =
            Vec::with_capacity(CONNECTIONS);
        let mut closed_at = Vec::with_capacity(CONNECTIONS);

        for _ in 0..CONNECTIONS {
            let app = Router::new().route("/", get(|| async { "ok" }));
            let (client, server) = io::duplex(1024);

            tokio::spawn(
                serve(ReadyListener(Some(server)), app)
                    .connection_lifetime_limits(
                        ConnectionLifetimeLimits::new()
                            .max_connection_age(MaxConnectionAge::new(age).jitter(jitter)),
                    )
                    .into_future(),
            );

            let stream = TokioIo::new(client);
            let (sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
            senders.push(sender);
            // Each connection records when it was closed from its own task, so
            // the deadlines don't collapse onto whichever one is awaited first.
            closed_at.push(tokio::spawn(async move {
                conn.await.ok();
                tokio::time::Instant::now()
            }));
        }

        let mut deadlines = Vec::with_capacity(CONNECTIONS);
        for handle in closed_at {
            let closed = tokio::time::timeout(Duration::from_secs(60), handle)
                .await
                .expect("connection was not closed after max_connection_age elapsed")
                .unwrap();
            deadlines.push(closed - start);
        }

        for deadline in &deadlines {
            assert!(
                *deadline >= age,
                "connection closed after {deadline:?}, before the age limit {age:?}",
            );
            assert!(
                *deadline <= age + jitter,
                "connection closed after {deadline:?}, past the jittered bound {:?}",
                age + jitter,
            );
        }

        // Every connection would close at exactly `age` if jitter were ignored.
        // Each draw is uniform over `[0, jitter]`, so all of them landing within
        // a second of zero is vanishingly unlikely.
        assert!(
            deadlines
                .iter()
                .any(|deadline| *deadline > age + Duration::from_secs(1)),
            "no connection had jitter added to its age limit: {deadlines:?}",
        );
    }

    // A shutdown signal only asks connections to stop accepting new requests, so
    // on its own it waits forever on a stuck handler. The age and grace limits
    // still apply, force-closing the connection and letting the `serve` future
    // resolve.
    #[tokio::test(start_paused = true)]
    async fn max_connection_age_grace_unblocks_graceful_shutdown() {
        use std::{future::pending, sync::Arc};

        use tokio::sync::{oneshot, Notify};

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

        let (signal_tx, signal_rx) = oneshot::channel::<()>();
        let server_handle = tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(ConnectionLifetimeLimits::new().max_connection_age(
                    MaxConnectionAge::new(Duration::from_secs(10)).grace(Duration::from_secs(5)),
                ))
                .with_graceful_shutdown(async move {
                    signal_rx.await.ok();
                })
                .into_future(),
        );

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let send = tokio::spawn(async move { sender.send_request(request).await });

        // Wait until the (never-completing) handler is actually running, then
        // signal shutdown well before the age limit fires.
        started.notified().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        signal_tx.send(()).unwrap();

        // The age limit still fires at 10s and the grace period force-closes at
        // 15s, so the in-flight request resolves with an error.
        let result = tokio::time::timeout(Duration::from_secs(60), send)
            .await
            .expect("request was not aborted within the grace period")
            .unwrap();
        assert!(
            result.is_err(),
            "expected the in-flight request to fail when the connection is force-closed",
        );

        tokio::time::timeout(Duration::from_secs(60), server_handle)
            .await
            .expect("serve future did not resolve after the connection was force-closed")
            .unwrap();
    }
}
