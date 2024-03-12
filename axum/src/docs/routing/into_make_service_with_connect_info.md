Convert this router into a [`MakeService`], that will store `C`'s
associated `ConnectInfo` in a request extension such that [`ConnectInfo`]
can extract it.

This enables extracting things like the client's remote address.

Extracting the listening network's `SocketAddr` type is supported out of the box:

```rust
use axum::{
    extract::ConnectInfo,
    routing::get,
    Router,
};
use std::net::SocketAddr;

let app = Router::new().route("/", get(handler));

async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
    format!("Hello {addr}")
}

# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
# };
```

You can implement custom a [`Connected`] like so:

```rust
use axum::{
    extract::connect_info::{ConnectInfo, Connected},
    routing::get,
    serve::IncomingStream,
    Router,
};

let app = Router::new().route("/", get(handler));

async fn handler(
    ConnectInfo(my_connect_info): ConnectInfo<MyConnectInfo>,
) -> String {
    format!("Hello {my_connect_info:?}")
}

#[derive(Clone, Debug)]
struct MyConnectInfo {
    // ...
}

impl<A: axum::LocalAddr, R> Connected<IncomingStream<'_, A, R>> for MyConnectInfo
where
    R: Clone + Send + Sync + 'static,
{
    type Addr = Self;

    fn connect_info(_target: IncomingStream<'_, A, R>) -> Self::Addr {
        MyConnectInfo {
            // ...
        }
    }
}

# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, app.into_make_service_with_connect_info::<MyConnectInfo>()).await.unwrap();
# };
```

See the [unix domain socket example][uds] for an example of how to use
this to collect UDS connection info.

[`MakeService`]: tower::make::MakeService
[`Connected`]: crate::extract::connect_info::Connected
[`ConnectInfo`]: crate::extract::connect_info::ConnectInfo
[uds]: https://github.com/tokio-rs/axum/blob/main/examples/unix-domain-socket/src/main.rs
