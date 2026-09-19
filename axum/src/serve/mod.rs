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
use futures_util::{
    future::{select, Either},
    FutureExt,
};
use http::{Method, StatusCode};
use http_body::{Body as HttpBody, Frame, SizeHint};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
#[cfg(feature = "http1")]
use hyper_util::rt::TokioTimer;
#[cfg(any(feature = "http1", feature = "http2"))]
use hyper_util::{server::conn::auto::Builder, service::TowerToHyperService};
use pin_project_lite::pin_project;
use tokio::{sync::watch, task::JoinHandle};
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
/// Two limits are available: [`max_connection_age`] caps how long a connection
/// lives, and [`max_connection_idle`] closes a connection that is not being
/// used. They can be set together.
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
/// let limits = ConnectionLifetimeLimits::new()
///     .max_connection_age(
///         MaxConnectionAge::new(Duration::from_secs(10 * 60))
///             // Random per-connection jitter added to the age, to avoid
///             // synchronized reconnect storms when many connections were
///             // established at once.
///             .jitter(Duration::from_secs(60))
///             // Hard cap on how long to wait for in-flight work after the age
///             // limit fires before forcibly closing.
///             .grace(Duration::from_secs(30)),
///     )
///     // Close a connection that has served nothing for five minutes.
///     .max_connection_idle(Duration::from_secs(5 * 60));
///
/// axum::serve(listener, router)
///     .connection_lifetime_limits(limits)
///     .await;
/// # };
/// ```
///
/// [`max_connection_age`]: ConnectionLifetimeLimits::max_connection_age
/// [`max_connection_idle`]: ConnectionLifetimeLimits::max_connection_idle
#[derive(Clone, Debug, Default)]
#[must_use]
pub struct ConnectionLifetimeLimits {
    max_connection_age: Option<MaxConnectionAge>,
    max_connection_idle: Option<Duration>,
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

    /// Close a connection that has had no request in flight for `idle`.
    ///
    /// The clock starts when the connection goes idle and is reset as soon as a
    /// request arrives. A connection that keeps serving requests is therefore
    /// never closed by this limit, however long it lives. That is what sets it
    /// apart from [`max_connection_age`], which caps the lifetime of busy
    /// connections too.
    ///
    /// A request counts as in flight until its response body has been fully
    /// sent, so a slow handler or a streaming response keeps the connection
    /// busy. For HTTP/2 the count covers every stream, so the connection is
    /// idle only while no stream is open.
    ///
    /// Once the limit elapses, a graceful shutdown of the connection is
    /// started, the same way [`MaxConnectionAge`] does it: HTTP/1 closes the
    /// connection after the current request, HTTP/2 sends a `GOAWAY`. There is
    /// no grace period, because an idle connection has no in-flight work to
    /// wait for.
    ///
    /// This releases the memory and the file descriptor of connections that a
    /// client holds open but does not use. The trade-off is latency: a client
    /// that pauses for longer than the limit pays for a new connection, and for
    /// a new TLS handshake, on its next request. Set the limit above the idle
    /// timeout of the clients you expect.
    ///
    /// Connection idle time is unbounded by default.
    ///
    /// # Requests that race the limit
    ///
    /// A request only counts as in flight once its headers have arrived in
    /// full. A connection whose limit elapses while a client is part way
    /// through sending a request is therefore closed under it, and that client
    /// sees the connection end with no response and no error.
    ///
    /// This is the race every HTTP/1.1 client already has to handle, because a
    /// connection it believes is reusable can always have been closed by the
    /// server at the same moment, and clients retry idempotent requests when it
    /// happens. What is particular to this limit is that the deadline falls
    /// precisely when a connection is idle, which is also when a connection
    /// pool is most likely to reach for it. Set the limit well above the idle
    /// timeout of the clients you expect, so that they retire connections
    /// before the server does, rather than relying on the window being narrow.
    ///
    /// # Upgraded connections
    ///
    /// This limit never closes a connection that has been upgraded, e.g. to a
    /// WebSocket or via `CONNECT`. An upgraded HTTP/1 connection is handed to
    /// the upgrade handler and [`serve`] stops tracking it. An upgraded HTTP/2
    /// stream stays on the tracked connection, and [`serve`] cannot see the
    /// traffic on it, so it counts as in flight for the rest of the
    /// connection's life.
    ///
    /// For HTTP/2 that outlasts the upgraded stream itself: the count is never
    /// given back, so a single WebSocket leaves this limit disabled on the
    /// connection that carried it even after that WebSocket has closed. Set
    /// [`max_connection_age`] as well if such connections still need a bound.
    /// A stream counts as upgraded once a `CONNECT` request is answered with a
    /// success status, so a handler that answers `CONNECT` without upgrading
    /// has the same effect.
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
    /// let limits =
    ///     ConnectionLifetimeLimits::new().max_connection_idle(Duration::from_secs(5 * 60));
    ///
    /// axum::serve(listener, router)
    ///     .connection_lifetime_limits(limits)
    ///     .await;
    /// # };
    /// ```
    ///
    /// [`max_connection_age`]: ConnectionLifetimeLimits::max_connection_age
    pub fn max_connection_idle(mut self, idle: Duration) -> Self {
        self.max_connection_idle = Some(idle);
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

/// Completes once a connection has had no request in flight for `limit`.
///
/// `in_flight` carries the number of requests the connection is currently
/// serving, published by [`TrackInFlight`]. The wait starts over every time a
/// request arrives, so a connection that keeps serving requests never reaches
/// the limit. Never completes if there is no limit to apply, in which case
/// there is no channel to watch either.
async fn idle_limit_elapsed(limit: Option<Duration>, in_flight: Option<watch::Receiver<usize>>) {
    let (Some(limit), Some(mut in_flight)) = (limit, in_flight) else {
        return std::future::pending().await;
    };

    // `changed` fails once the sender is dropped, which happens with the
    // service that holds it. The count can never change again then, and the
    // connection is already on its way out, so treat that like the limit being
    // reached rather than waiting forever.
    loop {
        // The `Ref` returned by `borrow_and_update` holds a read lock on the
        // value, so take a copy of the count before awaiting anything.
        let count = *in_flight.borrow_and_update();

        if count > 0 {
            if in_flight.changed().await.is_err() {
                break;
            }
            continue;
        }

        let sleep = pin!(tokio::time::sleep(limit));
        let arrived = pin!(in_flight.changed());
        match select(sleep, arrived).await {
            // A request arrived, so start the wait over.
            Either::Right((Ok(()), _)) => {}
            // The limit elapsed with nothing arriving.
            Either::Left(_) | Either::Right((Err(_), _)) => break,
        }
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
                &connection_lifetime_limits,
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

/// What asked a connection task to shut its connection down, other than the
/// connection finishing on its own.
enum ShutdownTrigger {
    /// The shutdown signal passed to [`Serve::with_graceful_shutdown`] fired.
    Signal,
    /// The age timer fired, which is the age limit the first time and the grace
    /// period after that.
    Age,
    /// The connection had no request in flight for the idle limit.
    Idle,
}

async fn handle_connection<L, M, S, B, E>(
    make_service: &mut M,
    signal_rx: &watch::Receiver<()>,
    close_rx: &watch::Receiver<()>,
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
    let mut signal_rx = signal_rx.clone();
    let connection_lifetime_limits = connection_lifetime_limits.clone();
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

    // Requests in flight on this connection: the service counts them up and
    // down, and the connection task treats a count of zero as idle. The channel
    // only exists when there is an idle limit to apply, so a connection without
    // one neither counts anything nor pays for the channel.
    let max_connection_idle = connection_lifetime_limits.max_connection_idle;
    let (in_flight_tx, in_flight_rx) = match max_connection_idle {
        Some(_) => {
            let (tx, rx) = watch::channel(0usize);
            (Some(Arc::new(tx)), Some(rx))
        }
        None => (None, None),
    };

    let hyper_service = TrackInFlight {
        inner: TowerToHyperService::new(tower_service),
        in_flight: in_flight_tx,
    };

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
        let max_connection_age = connection_lifetime_limits.max_connection_age;
        let max_age = max_connection_age
            .as_ref()
            .map(|limit| limit.age.saturating_add(random_duration(limit.jitter)));
        let grace = max_connection_age.and_then(|limit| limit.grace);
        let mut timer = pin!(sleep_or_pending(max_age));
        let mut age_fired = false;

        // Idle limit for the connection. Unlike the age timer this only runs
        // while nothing is in flight, so it never fires on a connection that is
        // still being used. It is fused because it fires at most once: after
        // that the connection is already shutting down gracefully.
        let mut idle = pin!(idle_limit_elapsed(max_connection_idle, in_flight_rx).fuse());

        loop {
            let trigger = match select(
                conn.as_mut(),
                select(
                    select(signal_closed.as_mut(), timer.as_mut()),
                    idle.as_mut(),
                ),
            )
            .await
            {
                Either::Left((result, _)) => {
                    if let Err(_err) = result {
                        trace!("failed to serve connection: {_err:#}");
                    }
                    break;
                }
                Either::Right((Either::Left((Either::Left((Err(_), _)), _)), _)) => {
                    ShutdownTrigger::Signal
                }
                Either::Right((Either::Left((Either::Left((Ok(()), _)), _)), _)) => {
                    unreachable!("shutdown channel never sends values")
                }
                Either::Right((Either::Left((Either::Right(_), _)), _)) => ShutdownTrigger::Age,
                Either::Right((Either::Right(_), _)) => ShutdownTrigger::Idle,
            };

            match trigger {
                ShutdownTrigger::Signal => {
                    trace!("signal received in task, starting graceful shutdown");
                    conn.as_mut().graceful_shutdown();
                }
                ShutdownTrigger::Age if !age_fired => {
                    age_fired = true;
                    trace!("max connection age reached, starting graceful shutdown");
                    conn.as_mut().graceful_shutdown();
                    timer.set(sleep_or_pending(grace));
                }
                ShutdownTrigger::Age => {
                    trace!("max connection age grace period elapsed, closing connection");
                    break;
                }
                ShutdownTrigger::Idle => {
                    trace!("max connection idle reached, starting graceful shutdown");
                    conn.as_mut().graceful_shutdown();
                }
            }
        }

        drop(close_rx);
    });
}

/// Publishes how many requests a connection is serving, so that its connection
/// task can tell when the connection is idle.
///
/// A request is counted from the moment the service is called until its
/// response body ends, rather than until the handler returns a response: a
/// streaming response is still being served long after that.
struct TrackInFlight<S> {
    inner: S,
    /// The count of requests in flight, or `None` when there is no idle limit
    /// and nothing needs counting.
    in_flight: Option<Arc<watch::Sender<usize>>>,
}

impl<S, B> hyper::service::Service<Request<Incoming>> for TrackInFlight<S>
where
    S: hyper::service::Service<Request<Incoming>, Response = Response<B>>,
{
    type Response = Response<TrackedBody<B>>;
    type Error = S::Error;
    type Future = TrackInFlightFuture<S::Future>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        TrackInFlightFuture {
            connect: req.method() == Method::CONNECT,
            guard: self.in_flight.clone().map(InFlightGuard::new),
            inner: self.inner.call(req),
        }
    }
}

pin_project! {
    /// Response future for [`TrackInFlight`].
    struct TrackInFlightFuture<F> {
        #[pin]
        inner: F,
        guard: Option<InFlightGuard>,
        // Whether the request used the `CONNECT` method, which together with a
        // successful response means the stream was handed to an upgrade.
        connect: bool,
    }
}

impl<F, B, E> Future for TrackInFlightFuture<F>
where
    F: Future<Output = Result<Response<B>, E>>,
{
    type Output = Result<Response<TrackedBody<B>>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let response = ready!(this.inner.poll(cx)?);

        // An upgraded stream keeps carrying traffic that `serve` cannot see, so
        // its count is never given back and the connection counts as busy for
        // the rest of its life. HTTP/1 answers an upgrade with `101`, HTTP/2
        // answers an extended `CONNECT` with a success status.
        let upgraded = response.status() == StatusCode::SWITCHING_PROTOCOLS
            || (*this.connect && response.status().is_success());
        if upgraded {
            if let Some(guard) = this.guard.as_mut() {
                guard.upgraded = true;
            }
        }

        Poll::Ready(Ok(response.map(|body| TrackedBody {
            inner: body,
            guard: this.guard.take(),
        })))
    }
}

pin_project! {
    /// Response body returned by [`TrackInFlight`], which counts its request as
    /// in flight until the body ends.
    struct TrackedBody<B> {
        #[pin]
        inner: B,
        guard: Option<InFlightGuard>,
    }
}

impl<B> HttpBody for TrackedBody<B>
where
    B: HttpBody,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        let frame = ready!(this.inner.poll_frame(cx));

        // The end of the body is the end of the request. A body that is dropped
        // part way, because the response failed or the client went away, drops
        // the guard instead and has the same effect.
        if frame.is_none() {
            *this.guard = None;
        }

        Poll::Ready(frame)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

/// Holds one request's place in the in-flight count for as long as it is alive.
struct InFlightGuard {
    in_flight: Arc<watch::Sender<usize>>,
    /// Set once the request's stream has been handed to an upgrade, in which
    /// case the count is never given back.
    upgraded: bool,
}

impl InFlightGuard {
    fn new(in_flight: Arc<watch::Sender<usize>>) -> Self {
        in_flight.send_modify(|count| *count += 1);
        Self {
            in_flight,
            upgraded: false,
        }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        if !self.upgraded {
            self.in_flight.send_modify(|count| *count -= 1);
        }
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
        serve(TcpListener::bind(addr).await.unwrap(), get(handler)).with_executor(exec.clone());
        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_make_service(),
        )
        .with_executor(exec);

        // connection_lifetime_limits, composable with the other builder methods in any order
        let limits = ConnectionLifetimeLimits::new()
            .max_connection_age(
                MaxConnectionAge::new(Duration::from_secs(60))
                    .jitter(Duration::from_secs(10))
                    .grace(Duration::from_secs(5)),
            )
            .max_connection_idle(Duration::from_secs(30));
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

    // After `max_connection_idle` elapses with nothing in flight, the
    // connection is gracefully shut down by the server, which the client
    // observes as its connection task completing.
    #[tokio::test(start_paused = true)]
    async fn max_connection_idle_closes_idle_connection() {
        let app = Router::new().route("/", get(|| async { "ok" }));
        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new().max_connection_idle(Duration::from_secs(10)),
                )
                .into_future(),
        );

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        let conn_handle = tokio::spawn(conn);

        // A first request succeeds normally before the connection goes idle.
        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(Body::new(response.into_body()), usize::MAX)
            .await
            .unwrap();

        // With (paused) time auto-advancing, the idle limit elapses and the
        // server closes the connection, completing the client's conn task.
        tokio::time::timeout(Duration::from_secs(30), conn_handle)
            .await
            .expect("connection was not closed after max_connection_idle elapsed")
            .unwrap()
            .ok();
    }

    // A connection that never sends a single byte is idle from the moment it is
    // accepted, so the limit closes it without a request ever being served.
    // This goes through a different path in hyper than a connection that has
    // served something: the protocol has not been determined yet, so there is
    // no HTTP/1 or HTTP/2 connection to shut down gracefully.
    #[tokio::test(start_paused = true)]
    async fn max_connection_idle_closes_a_connection_that_is_never_used() {
        use tokio::io::AsyncReadExt;

        let app = Router::new().route("/", get(|| async { "ok" }));
        let (mut client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new().max_connection_idle(Duration::from_secs(10)),
                )
                .into_future(),
        );

        // Never send anything, and read until the server hangs up.
        let mut buf = [0u8; 8];
        let read = tokio::time::timeout(Duration::from_secs(60), client.read(&mut buf))
            .await
            .expect("a connection that was never used was not closed by max_connection_idle")
            .unwrap();
        assert_eq!(read, 0, "expected EOF, got {read} bytes");
    }

    // A request that is in flight for longer than the idle limit keeps the
    // connection busy, so the limit never elapses and the response arrives.
    #[tokio::test(start_paused = true)]
    async fn max_connection_idle_waits_for_slow_handler() {
        // The handler takes 30s, six times the 5s idle limit.
        let app = Router::new().route(
            "/",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                "done"
            }),
        );

        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new().max_connection_idle(Duration::from_secs(5)),
                )
                .into_future(),
        );

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = tokio::time::timeout(Duration::from_secs(60), sender.send_request(request))
            .await
            .expect("in-flight request did not resolve")
            .expect("in-flight request failed: a busy connection must not count as idle");
        assert_eq!(response.status(), StatusCode::OK);
    }

    // A request is in flight until its response body has been fully sent, not
    // until the handler returns a response, so a response that streams for
    // longer than the idle limit still arrives in full.
    #[tokio::test(start_paused = true)]
    async fn max_connection_idle_waits_for_streaming_response_body() {
        use futures_util::stream;

        const CHUNK: &str = "chunk";
        const CHUNKS: usize = 5;

        // Five chunks five seconds apart span 25s, five times the 5s idle
        // limit. The handler itself returns straight away.
        let app = Router::new().route(
            "/",
            get(|| async {
                Body::from_stream(stream::unfold(0usize, |sent| async move {
                    if sent == CHUNKS {
                        return None;
                    }
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    Some((Ok::<_, std::io::Error>(CHUNK), sent + 1))
                }))
            }),
        );

        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new().max_connection_idle(Duration::from_secs(5)),
                )
                .into_future(),
        );

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = tokio::time::timeout(
            Duration::from_secs(60),
            to_bytes(Body::new(response.into_body()), usize::MAX),
        )
        .await
        .expect("streaming response did not finish")
        .expect("streaming response was cut short");
        assert_eq!(body.len(), CHUNKS * CHUNK.len());

        // The connection stayed open through all of it. It would have been
        // closed after the streaming response if the idle limit had elapsed
        // while the body was still being sent.
        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = sender
            .send_request(request)
            .await
            .expect("connection was closed while a response body was still streaming");
        assert_eq!(response.status(), StatusCode::OK);
    }

    // The HTTP/2 equivalent of `max_connection_idle_closes_idle_connection`:
    // the server sends GOAWAY once the idle limit elapses, completing the
    // client's connection task.
    #[cfg(feature = "http2")]
    #[tokio::test(start_paused = true)]
    async fn max_connection_idle_closes_idle_connection_http2() {
        use hyper_util::rt::TokioExecutor;

        let app = Router::new().route("/", get(|| async { "ok" }));
        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new().max_connection_idle(Duration::from_secs(10)),
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

        tokio::time::timeout(Duration::from_secs(30), conn_handle)
            .await
            .expect("HTTP/2 connection was not closed after max_connection_idle elapsed")
            .unwrap()
            .ok();
    }

    // HTTP/2 multiplexes, so a connection is idle only while no stream is open,
    // not once the most recent request finished. A connection with one stream
    // still open is never shut down, however long the other streams have been
    // done for.
    #[cfg(feature = "http2")]
    #[tokio::test(start_paused = true)]
    async fn max_connection_idle_waits_for_open_stream_http2() {
        use std::sync::Arc;

        use hyper_util::rt::TokioExecutor;
        use tokio::sync::Notify;

        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());

        let app = Router::new()
            .route(
                "/blocked",
                get({
                    let started = started.clone();
                    let release = release.clone();
                    move || {
                        let started = started.clone();
                        let release = release.clone();
                        async move {
                            started.notify_one();
                            release.notified().await;
                            "blocked"
                        }
                    }
                }),
            )
            .route("/quick", get(|| async { "quick" }));

        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new().max_connection_idle(Duration::from_secs(5)),
                )
                .into_future(),
        );

        let io = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http2::Builder::new(TokioExecutor::new())
            .handshake(io)
            .await
            .unwrap();
        tokio::spawn(conn);

        // Open a stream that stays open, and wait until the server is serving
        // it.
        let blocked = {
            let mut sender = sender.clone();
            let request = Request::builder()
                .uri("/blocked")
                .body(Body::empty())
                .unwrap();
            tokio::spawn(async move { sender.send_request(request).await })
        };
        started.notified().await;

        // A second stream that opens and closes again while the first one is
        // open. The connection is still busy afterwards, because one stream is
        // still open.
        let request = Request::builder()
            .uri("/quick")
            .body(Body::empty())
            .unwrap();
        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(Body::new(response.into_body()), usize::MAX)
            .await
            .unwrap();

        // Well past the idle limit, the connection must still accept new
        // streams: it would refuse them after a GOAWAY.
        tokio::time::sleep(Duration::from_secs(30)).await;
        let request = Request::builder()
            .uri("/quick")
            .body(Body::empty())
            .unwrap();
        let response = sender
            .send_request(request)
            .await
            .expect("connection was shut down while a stream was still open");
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(Body::new(response.into_body()), usize::MAX)
            .await
            .unwrap();

        release.notify_one();
        let response = tokio::time::timeout(Duration::from_secs(30), blocked)
            .await
            .expect("the open stream did not resolve")
            .unwrap()
            .expect("the open stream failed");
        assert_eq!(response.status(), StatusCode::OK);
    }

    // An upgraded HTTP/1 connection is handed to the upgrade handler, and the
    // traffic on it is invisible to `serve`, so a WebSocket that says nothing
    // for longer than the idle limit must not be closed under it.
    #[cfg(all(feature = "http1", feature = "ws"))]
    #[tokio::test(start_paused = true)]
    async fn max_connection_idle_never_closes_an_upgraded_connection_http1() {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::{tungstenite, WebSocketStream};

        use crate::{
            extract::ws::{Message, WebSocketUpgrade},
            routing::any,
        };

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
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new().max_connection_idle(Duration::from_secs(5)),
                )
                .into_future(),
        );

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

        let upgraded = hyper::upgrade::on(&mut response).await.unwrap();
        let mut ws = WebSocketStream::from_raw_socket(
            TokioIo::new(upgraded),
            tungstenite::protocol::Role::Client,
            None,
        )
        .await;

        // Say nothing for well past the idle limit, then check the socket is
        // still there.
        tokio::time::sleep(Duration::from_secs(60)).await;

        ws.send(tungstenite::Message::Text("hi".into()))
            .await
            .expect("upgraded HTTP/1 connection was closed by the idle limit");
        let echoed = tokio::time::timeout(Duration::from_secs(30), ws.next())
            .await
            .expect("no echo from the upgraded HTTP/1 connection")
            .expect("the upgraded HTTP/1 connection ended")
            .expect("the upgraded HTTP/1 connection failed");
        assert_eq!(echoed.into_text().unwrap().as_str(), "hi");
    }

    // An upgraded HTTP/2 stream keeps carrying traffic that `serve` cannot see,
    // so it counts as in flight for the rest of the connection's life and the
    // idle limit never closes the connection under it.
    #[cfg(all(feature = "http2", feature = "ws"))]
    #[tokio::test(start_paused = true)]
    async fn max_connection_idle_never_closes_an_upgraded_connection_http2() {
        use hyper_util::rt::TokioExecutor;

        use crate::{extract::ws::WebSocketUpgrade, routing::any};

        let app = Router::new()
            .route(
                "/ws",
                any(|ws: WebSocketUpgrade| async move {
                    ws.on_upgrade(
                        |mut socket| async move { while socket.recv().await.is_some() {} },
                    )
                }),
            )
            .route("/", get(|| async { "ok" }));

        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new().max_connection_idle(Duration::from_secs(5)),
                )
                .into_future(),
        );

        let io = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http2::Builder::new(TokioExecutor::new())
            .handshake(io)
            .await
            .unwrap();

        // Wait a little for the SETTINGS frame that advertises extended
        // CONNECT to go through.
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert!(conn.is_extended_connect_protocol_enabled());
        tokio::spawn(conn);

        let request = http::Request::builder()
            .method(http::Method::CONNECT)
            .extension(hyper::ext::Protocol::from_static("websocket"))
            .uri("/ws")
            .header("sec-websocket-version", "13")
            .body(Body::empty())
            .unwrap();
        let mut response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Hold the upgraded stream open without sending anything on it.
        let _upgraded = hyper::upgrade::on(&mut response).await.unwrap();

        // Well past the idle limit, the connection must still accept new
        // streams: it would refuse them after a GOAWAY.
        tokio::time::sleep(Duration::from_secs(30)).await;
        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = sender
            .send_request(request)
            .await
            .expect("connection was shut down under an upgraded stream");
        assert_eq!(response.status(), StatusCode::OK);
    }

    // With both limits set, a connection that goes idle is closed by the idle
    // limit long before it reaches the age limit.
    #[tokio::test(start_paused = true)]
    async fn max_connection_idle_closes_connection_before_max_connection_age() {
        let age = Duration::from_secs(10 * 60);
        let idle = Duration::from_secs(5);

        let app = Router::new().route("/", get(|| async { "ok" }));
        let (client, server) = io::duplex(1024);
        let listener = ReadyListener(Some(server));

        tokio::spawn(
            serve(listener, app)
                .connection_lifetime_limits(
                    ConnectionLifetimeLimits::new()
                        .max_connection_age(MaxConnectionAge::new(age))
                        .max_connection_idle(idle),
                )
                .into_future(),
        );

        let start = tokio::time::Instant::now();
        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        let conn_handle = tokio::spawn(conn);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(Body::new(response.into_body()), usize::MAX)
            .await
            .unwrap();

        tokio::time::timeout(age, conn_handle)
            .await
            .expect("connection was not closed after max_connection_idle elapsed")
            .unwrap()
            .ok();

        let closed_after = start.elapsed();
        assert!(
            closed_after < age,
            "connection closed after {closed_after:?}, which is the age limit {age:?} rather \
             than the idle limit {idle:?}",
        );
    }

    // With both limits set, the age limit still bounds a connection that never
    // goes idle: the idle limit alone would let a stuck handler hold it open
    // forever.
    #[tokio::test(start_paused = true)]
    async fn max_connection_age_still_applies_to_a_connection_that_is_never_idle() {
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
                        .max_connection_age(
                            MaxConnectionAge::new(Duration::from_secs(10))
                                .grace(Duration::from_secs(5)),
                        )
                        .max_connection_idle(Duration::from_secs(60)),
                )
                .into_future(),
        );

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let send = tokio::spawn(async move { sender.send_request(request).await });

        // Wait until the (never-completing) handler is actually running, which
        // keeps the connection busy for the rest of the test.
        started.notified().await;

        // age (10s) + grace (5s) later, the connection is force-closed, so the
        // in-flight request resolves with an error.
        let result = tokio::time::timeout(Duration::from_secs(30), send)
            .await
            .expect("request was not aborted within the grace period")
            .unwrap();
        assert!(
            result.is_err(),
            "expected the in-flight request to fail when the connection is force-closed",
        );
    }
}
