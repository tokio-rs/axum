//! Extractor for getting connection information from a client.
//!
//! See [`Router::into_make_service_with_connect_info`] for more details.
//!
//! [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info

use super::{Extension, FromRequest, RequestParts};
use crate::{AddExtension, AddExtensionLayer};
use async_trait::async_trait;
use hyper::server::conn::AddrStream;
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    fmt,
    future::{ready, Future},
    marker::PhantomData,
    net::SocketAddr,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// A [`MakeService`] created from a router.
///
/// See [`Router::into_make_service_with_connect_info`] for more details.
///
/// [`MakeService`]: tower::make::MakeService
/// [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info
pub struct WithConnectInfo<M, C> {
    make_svc: M,
    _connect_info: PhantomData<fn() -> C>,
}

#[test]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<WithConnectInfo<(), NotSendSync>>();
}

impl<M, C> WithConnectInfo<M, C> {
    pub(crate) fn new(make_svc: M) -> Self {
        Self {
            make_svc,
            _connect_info: PhantomData,
        }
    }
}

impl<M, C> fmt::Debug for WithConnectInfo<M, C>
where
    M: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoMakeServiceWithConnectInfo")
            .field("svc", &self.make_svc)
            .finish()
    }
}

impl<M, C> Clone for WithConnectInfo<M, C>
where
    M: Clone,
{
    fn clone(&self) -> Self {
        Self {
            make_svc: self.make_svc.clone(),
            _connect_info: PhantomData,
        }
    }
}

/// Trait that connected IO resources implement and use to produce information
/// about the connection.
///
/// The goal for this trait is to allow users to implement custom IO types that
/// can still provide the same connection metadata.
///
/// See [`Router::into_make_service_with_connect_info`] for more details.
///
/// [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info
pub trait Connected<T>: Clone + Send + Sync + 'static {
    /// Create type holding information about the connection.
    fn connect_info(target: &T) -> Self;
}

impl Connected<&AddrStream> for SocketAddr {
    fn connect_info(target: &&AddrStream) -> Self {
        target.remote_addr()
    }
}

impl<M, C, T> Service<T> for WithConnectInfo<M, C>
where
    M: Service<T>,
    C: Connected<T>,
{
    type Response = AddExtension<M::Response, ConnectInfo<C>>;
    type Error = M::Error;
    type Future = ResponseFuture<M::Future, C>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.make_svc.poll_ready(cx)
    }

    fn call(&mut self, target: T) -> Self::Future {
        let connect_info = C::connect_info(&target);
        ResponseFuture {
            future: self.make_svc.call(target),
            connect_info: Some(connect_info),
        }
    }
}

pin_project! {
    /// Response future for [`IntoMakeServiceWithConnectInfo`].
    pub struct ResponseFuture<F, C> {
        #[pin]
        future: F,
        connect_info: Option<C>,
    }
}

impl<F, C, S, E> Future for ResponseFuture<F, C>
where
    F: Future<Output = Result<S, E>>,
    C: Clone,
{
    type Output = Result<AddExtension<S, ConnectInfo<C>>, E>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let svc = futures_util::ready!(this.future.poll(cx))?;
        let connect_info = this
            .connect_info
            .take()
            .expect("future polled after completion");
        let svc = AddExtensionLayer::new(ConnectInfo(connect_info)).layer(svc);
        Poll::Ready(Ok(svc))
    }
}

/// Extractor for getting connection information produced by a [`Connected`].
///
/// Note this extractor requires you to use
/// [`Router::into_make_service_with_connect_info`] to run your app
/// otherwise it will fail at runtime.
///
/// See [`Router::into_make_service_with_connect_info`] for more details.
///
/// [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info
#[derive(Clone, Copy, Debug)]
pub struct ConnectInfo<T>(pub T);

#[async_trait]
impl<B, T> FromRequest<B> for ConnectInfo<T>
where
    B: Send,
    T: Clone + Send + Sync + 'static,
{
    type Rejection = <Extension<Self> as FromRequest<B>>::Rejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Extension(connect_info) = Extension::<Self>::from_request(req).await?;
        Ok(connect_info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{routing::get, Router, Server};
    use std::net::{SocketAddr, TcpListener};

    #[tokio::test]
    async fn socket_addr() {
        async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
            format!("{}", addr)
        }

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let app = Router::new().route("/", get(handler));
            let server = Server::from_tcp(listener)
                .unwrap()
                .serve(app.into_make_service().with_connect_info::<SocketAddr, _>());
            tx.send(()).unwrap();
            server.await.expect("server error");
        });
        rx.await.unwrap();

        let client = reqwest::Client::new();

        let res = client.get(format!("http://{}", addr)).send().await.unwrap();
        let body = res.text().await.unwrap();
        assert!(body.starts_with("127.0.0.1:"));
    }

    #[tokio::test]
    async fn custom() {
        #[derive(Clone, Debug)]
        struct MyConnectInfo {
            value: &'static str,
        }

        impl Connected<&AddrStream> for MyConnectInfo {
            fn connect_info(_target: &&AddrStream) -> Self {
                Self {
                    value: "it worked!",
                }
            }
        }

        async fn handler(ConnectInfo(addr): ConnectInfo<MyConnectInfo>) -> &'static str {
            addr.value
        }

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let app = Router::new().route("/", get(handler));
            let server = Server::from_tcp(listener).unwrap().serve(
                app.into_make_service()
                    .with_connect_info::<MyConnectInfo, _>(),
            );
            tx.send(()).unwrap();
            server.await.expect("server error");
        });
        rx.await.unwrap();

        let client = reqwest::Client::new();

        let res = client.get(format!("http://{}", addr)).send().await.unwrap();
        let body = res.text().await.unwrap();
        assert_eq!(body, "it worked!");
    }
}
