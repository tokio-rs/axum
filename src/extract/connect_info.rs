#![allow(missing_docs)]

use crate::extract::{FromRequest, RequestParts};
use async_trait::async_trait;
use hyper::server::conn::AddrStream;
use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::task::{Context, Poll};
use tower::Service;
use tower_http::add_extension::AddExtension;

use super::Extension;

pub struct IntoMakeServiceWithConnectInfo<S, C> {
    svc: S,
    _connect_info: PhantomData<fn() -> C>,
}

impl<S, C> IntoMakeServiceWithConnectInfo<S, C> {
    pub(crate) fn new(svc: S) -> Self {
        Self {
            svc,
            _connect_info: PhantomData,
        }
    }
}

impl<S, C> fmt::Debug for IntoMakeServiceWithConnectInfo<S, C>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoMakeServiceWithConnectInfo")
            .field("svc", &self.svc)
            .finish()
    }
}

pub trait Connected<T> {
    type ConnectInfo: Clone + Send + Sync + 'static;

    fn connect_info(target: &T) -> Self::ConnectInfo;
}

impl<S, C, T> Service<T> for IntoMakeServiceWithConnectInfo<S, C>
where
    S: Clone,
    C: Connected<T>,
{
    type Response = AddExtension<S, ConnectInfo<C::ConnectInfo>>;
    type Error = Infallible;
    type Future = futures_util::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, target: T) -> Self::Future {
        let connect_info = ConnectInfo(C::connect_info(&target));
        let svc = AddExtension::new(self.svc.clone(), connect_info);
        futures_util::future::ok(svc)
    }
}

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

impl Connected<&AddrStream> for SocketAddr {
    type ConnectInfo = SocketAddr;

    fn connect_info(target: &&AddrStream) -> Self::ConnectInfo {
        target.remote_addr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use hyper::Server;
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
            let app = route("/", get(handler));
            let server = Server::from_tcp(listener)
                .unwrap()
                .serve(app.into_make_service_with_connect_info::<SocketAddr, _>());
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
            type ConnectInfo = MyConnectInfo;

            fn connect_info(_target: &&AddrStream) -> Self::ConnectInfo {
                MyConnectInfo {
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
            let app = route("/", get(handler));
            let server = Server::from_tcp(listener)
                .unwrap()
                .serve(app.into_make_service_with_connect_info::<MyConnectInfo, _>());
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
