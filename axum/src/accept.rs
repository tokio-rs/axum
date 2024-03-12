use std::{fmt::Debug, sync::Arc};

use async_trait::async_trait;
use hyper::rt::{Read, Write};
#[cfg(feature = "tokio")]
use hyper_util::rt::TokioIo;
#[cfg(all(unix, feature = "tokio"))]
use tokio::net::{unix::UCred, UnixListener, UnixStream};
#[cfg(feature = "tokio")]
use tokio::net::{TcpListener, TcpStream};

/// A trait that provides a generic API for accepting connections
/// from any server type which listens on an address of some kind
#[async_trait]
pub trait Accept: Send + Sync + 'static {
    /// When a connection is accepted from the Listener type, it must
    /// be transformed into the Target type so that [`hyper`] can Read
    /// from it and Write to it
    type Target: Read + Write + LocalAddr + Unpin + Send + Sync;
    /// The SocketAddr associated with the given Listener type
    type Addr: Debug + Send + Sync + Clone;

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

#[derive(Clone, Debug)]
#[allow(dead_code)]
#[cfg(all(unix, feature = "tokio"))]
/// The associate SocketAddr type for the [`UnixStream`] type
pub struct UdsConnectInfo {
    /// Contains the path to the unix socket
    pub peer_addr: Arc<tokio::net::unix::SocketAddr>,
    /// Information like user started, pid, and gid
    pub peer_cred: UCred,
}

#[async_trait]
#[cfg(all(unix, feature = "tokio"))]
impl Accept for UnixListener {
    type Target = TokioIo<UnixStream>;
    type Addr = UdsConnectInfo;

    async fn accept(&self) -> std::io::Result<(Self::Target, Self::Addr)> {
        self.accept().await.and_then(|t| {
            let peer_cred = t.0.peer_cred()?;
            Ok((
                TokioIo::new(t.0),
                UdsConnectInfo {
                    peer_addr: Arc::new(t.1),
                    peer_cred,
                },
            ))
        })
    }
}

#[cfg(feature = "tokio")]
impl LocalAddr for TokioIo<TcpStream> {
    type Addr = std::net::SocketAddr;

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.inner().local_addr()
    }
}

#[cfg(all(unix, feature = "tokio"))]
impl LocalAddr for TokioIo<UnixStream> {
    type Addr = tokio::net::unix::SocketAddr;

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.inner().local_addr()
    }
}
