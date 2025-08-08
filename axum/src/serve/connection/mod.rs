use std::{
    error::Error as StdError,
    future::Future,
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
};

pub use hyper::{Hyper, HyperConnection};

#[cfg(any(feature = "http1", feature = "http2"))]
mod hyper;

/// Types that can handle connections accepted by a [`Listener`].
///
/// [`Listener`]: crate::serve::Listener
pub trait ConnectionBuilder<Io, S>: Clone {
    /// Take an accepted connection from the [`Listener`] (for example a `TcpStream`) and handle
    /// requests on it using the provided service (usually a [`Router`](crate::Router)).
    ///
    /// [`Listener`]: crate::serve::Listener
    fn build_connection(&mut self, io: Io, service: S) -> impl Connection;

    /// Signal to all ongoing connections that the server is shutting down.
    fn graceful_shutdown(&mut self);
}

/// A connection returned by [`ConnectionBuilder`].
///
/// This type must be driven by calling [`Connection::poll_connection`].
///
/// Note that each [`Connection`] may handle multiple requests.
pub trait Connection: Send {
    /// Poll the connection.
    fn poll_connection(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Box<dyn StdError + Send + Sync>>>;
}

impl<Ptr, Fut> Connection for Pin<Ptr>
where
    Ptr: DerefMut<Target = Fut> + Send,
    Fut: Future<Output = Result<(), Box<dyn StdError + Send + Sync>>> + Send,
{
    fn poll_connection(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Box<dyn StdError + Send + Sync>>> {
        self.as_mut().poll(cx)
    }
}
