use std::{fmt::Debug, path::PathBuf};

use async_trait::async_trait;
use hyper::rt::{Read, Write};
#[cfg(feature = "tokio")]
use hyper_util::rt::TokioIo;
#[cfg(feature = "tokio")]
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};

/// A trait that provides a generic API for accepting connections
/// from any server type which listens on an address of some kind
#[async_trait]
pub trait Accept: Send {
    /// When a connection is accepted from the Listener type, it must
    /// be transformed into the Target type so that [`hyper`] can Read
    /// from it and Write to it
    type Target: Read + Write + LocalAddr + Unpin + Send;
    /// The SocketAddr associated with the given Listener type
    type Addr: Debug + Send;

    /// Accept a new incoming connection from this Listener
    async fn accept(&self) -> std::io::Result<(Self::Target, Self::Addr)>;
}

/// Gets the local SocketAddr off the given type
pub trait LocalAddr {
    /// The SocketAddr associated with the given type
    type Addr;

    /// Calls local_addr on the given type
    fn local_addr(&self) -> std::io::Result<Self::Addr>;
}

#[async_trait]
#[cfg(feature = "tokio")]
impl Accept for TcpListener {
    type Target = TokioIo<TcpStream>;
    type Addr = std::net::SocketAddr;

    async fn accept(&self) -> std::io::Result<(Self::Target, Self::Addr)> {
        self.accept().await.map(|t| (TokioIo::new(t.0), t.1))
    }
}

#[async_trait]
#[cfg(feature = "tokio")]
impl Accept for UnixListener {
    type Target = TokioIo<UnixStream>;
    type Addr = Option<PathBuf>;

    async fn accept(&self) -> std::io::Result<(Self::Target, Self::Addr)> {
        self.accept()
            .await
            .map(|t| (TokioIo::new(t.0), t.1.as_pathname().map(|p| p.into())))
    }
}

#[cfg(feature = "tokio")]
impl LocalAddr for TokioIo<TcpStream> {
    type Addr = std::net::SocketAddr;

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.inner().local_addr()
    }
}

#[cfg(feature = "tokio")]
impl LocalAddr for TokioIo<UnixStream> {
    type Addr = tokio::net::unix::SocketAddr;

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.inner().local_addr()
    }
}
