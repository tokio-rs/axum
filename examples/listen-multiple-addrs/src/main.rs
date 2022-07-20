//! Showcases how listening on multiple addrs is possible by
//! implementing Accept for a custom struct.
//!
//! This may be useful in cases where the platform does not
//! listen on both IPv4 and IPv6 when the IPv6 catch-all listener is used (`::`),
//! [like older versions of Windows.](https://docs.microsoft.com/en-us/windows/win32/winsock/dual-stack-sockets)

use axum::{routing::get, Router};
use hyper::server::{accept::Accept, conn::AddrIncoming};
use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    let localhost_v4 = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8080);
    let incoming_v4 = AddrIncoming::bind(&localhost_v4).unwrap();

    let localhost_v6 = SocketAddr::new(Ipv6Addr::LOCALHOST.into(), 8080);
    let incoming_v6 = AddrIncoming::bind(&localhost_v6).unwrap();

    let combined = CombinedIncoming {
        a: incoming_v4,
        b: incoming_v6,
    };

    axum::Server::builder(combined)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

struct CombinedIncoming {
    a: AddrIncoming,
    b: AddrIncoming,
}

impl Accept for CombinedIncoming {
    type Conn = <AddrIncoming as Accept>::Conn;
    type Error = <AddrIncoming as Accept>::Error;

    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        if let Poll::Ready(Some(value)) = Pin::new(&mut self.a).poll_accept(cx) {
            return Poll::Ready(Some(value));
        }

        if let Poll::Ready(Some(value)) = Pin::new(&mut self.b).poll_accept(cx) {
            return Poll::Ready(Some(value));
        }

        Poll::Pending
    }
}
