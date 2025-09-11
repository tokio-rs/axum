use std::{
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use tokio::{
    io::{self, AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
};

/// Types that can listen for connections.
pub trait Listener: Send + 'static {
    /// The listener's IO type.
    type Io: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    /// The listener's address type.
    type Addr: Send;

    /// Accept a new incoming connection to this listener.
    ///
    /// If the underlying accept call can return an error, this function must
    /// take care of logging and retrying.
    fn accept(&mut self) -> impl Future<Output = (Self::Io, Self::Addr)> + Send;

    /// Returns the local address that this listener is bound to.
    fn local_addr(&self) -> io::Result<Self::Addr>;
}

impl Listener for TcpListener {
    type Io = TcpStream;
    type Addr = std::net::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match Self::accept(self).await {
                Ok(tup) => return tup,
                Err(e) => handle_accept_error(e).await,
            }
        }
    }

    #[inline]
    fn local_addr(&self) -> io::Result<Self::Addr> {
        Self::local_addr(self)
    }
}

#[cfg(unix)]
impl Listener for tokio::net::UnixListener {
    type Io = tokio::net::UnixStream;
    type Addr = tokio::net::unix::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match Self::accept(self).await {
                Ok(tup) => return tup,
                Err(e) => handle_accept_error(e).await,
            }
        }
    }

    #[inline]
    fn local_addr(&self) -> io::Result<Self::Addr> {
        Self::local_addr(self)
    }
}

/// Extensions to [`Listener`].
pub trait ListenerExt: Listener + Sized {
    /// Run a mutable closure on every accepted `Io`.
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{Router, routing::get, serve::ListenerExt};
    /// use tracing::trace;
    ///
    /// # async {
    /// let router = Router::new().route("/", get(|| async { "Hello, World!" }));
    ///
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
    ///     .await
    ///     .unwrap()
    ///     .tap_io(|tcp_stream| {
    ///         if let Err(err) = tcp_stream.set_nodelay(true) {
    ///             trace!("failed to set TCP_NODELAY on incoming connection: {err:#}");
    ///         }
    ///     });
    /// axum::serve(listener, router).await.unwrap();
    /// # };
    /// ```
    fn tap_io<F>(self, tap_fn: F) -> TapIo<Self, F>
    where
        F: FnMut(&mut Self::Io) + Send + 'static,
    {
        TapIo {
            listener: self,
            tap_fn,
        }
    }

    /// Add an async handshaking step to the listener.
    ///
    /// This is useful for implementing TLS. The handshaker closure is handed
    /// ownership of the I/O object and is expected to return a new I/O
    /// object, possibly of a different type.
    fn handshake<F, H, HandshakeIo>(self, handshaker: F) -> Handshake<Self, F, H, HandshakeIo>
    where
        F: FnMut(Self::Io) -> H + Send + 'static,
        H: Future<Output = Option<HandshakeIo>> + Send,
        HandshakeIo: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        Handshake {
            listener: self,
            handshaker,
            _phantom: PhantomData,
        }
    }
}

impl<L: Listener> ListenerExt for L {}

/// Return type of [`ListenerExt::tap_io`].
///
/// See that method for details.
pub struct TapIo<L, F> {
    listener: L,
    tap_fn: F,
}

impl<L, F> fmt::Debug for TapIo<L, F>
where
    L: Listener + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TapIo")
            .field("listener", &self.listener)
            .finish_non_exhaustive()
    }
}

impl<L, F> Listener for TapIo<L, F>
where
    L: Listener,
    F: FnMut(&mut L::Io) + Send + 'static,
{
    type Io = L::Io;
    type Addr = L::Addr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let (mut io, addr) = self.listener.accept().await;
        (self.tap_fn)(&mut io);
        (io, addr)
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.listener.local_addr()
    }
}

/// Return type of [`ListenerExt::handshake`].
///
/// See that method for details.
pub struct Handshake<L, F, H, HandshakeIo> {
    listener: L,
    handshaker: F,
    _phantom: PhantomData<fn() -> (H, HandshakeIo)>,
}

impl<L, F, H, HandshakeIo> fmt::Debug for Handshake<L, F, H, HandshakeIo>
where
    L: Listener + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handshake")
            .field("listener", &self.listener)
            .finish_non_exhaustive()
    }
}

impl<L, F, H, HandshakeIo> Listener for Handshake<L, F, H, HandshakeIo>
where
    L: Listener,
    F: FnMut(L::Io) -> H + Send + 'static,
    H: Future<Output = Option<HandshakeIo>> + Send + 'static,
    HandshakeIo: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Io = HandshakeFuture<Pin<Box<H>>, HandshakeIo>;
    type Addr = L::Addr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let (io, addr) = self.listener.accept().await;
        let handshake_fut = (self.handshaker)(io);

        (HandshakeFuture::new(Box::pin(handshake_fut)), addr)
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.listener.local_addr()
    }
}

pin_project_lite::pin_project! {
    pub struct HandshakeFuture<F, T>
    where
        F: Future,
    {
        #[pin]
        state: State<F, T>,
    }
}

pin_project_lite::pin_project! {
    #[project = StateProj]
    enum State<F, T> {
        Handshaking { #[pin] fut: F },
        Ready { stream: T },
    }
}

impl<F, T> HandshakeFuture<F, T>
where
    F: Future<Output = Option<T>>,
    T: AsyncRead + AsyncWrite,
{
    fn new(future: F) -> Self {
        Self {
            state: State::Handshaking { fut: future },
        }
    }
}

impl<F, T> AsyncRead for HandshakeFuture<F, T>
where
    F: Future<Output = Option<T>>,
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                StateProj::Handshaking { fut } => {
                    // Poll the handshake future
                    let stream = match fut.poll(cx) {
                        Poll::Ready(Some(stream)) => stream,
                        Poll::Ready(None) => {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::ConnectionAborted,
                                "handshake failed",
                            )));
                        }
                        Poll::Pending => return Poll::Pending,
                    };

                    // Handshake is complete, transition state to Ready and loop
                    // to poll the read on the new stream immediately.
                    this.state.set(State::Ready { stream });
                }
                StateProj::Ready { stream } => {
                    return Pin::new(stream).poll_read(cx, buf);
                }
            }
        }
    }
}

impl<F, T> AsyncWrite for HandshakeFuture<F, T>
where
    F: Future<Output = Option<T>>,
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut this = self.project();
        loop {
            match this.state.as_mut().project() {
                StateProj::Handshaking { fut } => {
                    let stream = match fut.poll(cx) {
                        Poll::Ready(Some(stream)) => stream,
                        Poll::Ready(None) => {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::ConnectionAborted,
                                "handshake failed",
                            )))
                        }
                        Poll::Pending => return Poll::Pending,
                    };
                    this.state.set(State::Ready { stream });
                }
                StateProj::Ready { stream } => {
                    return Pin::new(stream).poll_write(cx, buf);
                }
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut this = self.project();
        loop {
            match this.state.as_mut().project() {
                StateProj::Handshaking { fut } => {
                    let stream = match fut.poll(cx) {
                        Poll::Ready(Some(stream)) => stream,
                        Poll::Ready(None) => {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::ConnectionAborted,
                                "handshake failed",
                            )))
                        }
                        Poll::Pending => return Poll::Pending,
                    };
                    this.state.set(State::Ready { stream });
                }
                StateProj::Ready { stream } => {
                    return Pin::new(stream).poll_flush(cx);
                }
            }
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut this = self.project();
        loop {
            match this.state.as_mut().project() {
                StateProj::Handshaking { fut } => {
                    let stream = match fut.poll(cx) {
                        Poll::Ready(Some(stream)) => stream,
                        Poll::Ready(None) => {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::ConnectionAborted,
                                "handshake failed",
                            )))
                        }
                        Poll::Pending => return Poll::Pending,
                    };
                    this.state.set(State::Ready { stream });
                }
                StateProj::Ready { stream } => {
                    return Pin::new(stream).poll_shutdown(cx);
                }
            }
        }
    }
}

async fn handle_accept_error(e: io::Error) {
    if is_connection_error(&e) {
        return;
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
}

fn is_connection_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}
