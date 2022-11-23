Convert this router into a [`MakeService`], that will store `C`'s
associated `ConnectInfo` in a request extension such that [`ConnectInfo`]
can extract it.

This enables extracting things like the client's remote address.

Extracting [`std::net::SocketAddr`] is supported out of the box:

```rust
use axum::{
    extract::ConnectInfo,
    routing::get,
    Router,
};
use std::net::SocketAddr;

let app = Router::new().route("/", get(handler));

async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
    format!("Hello {}", addr)
}

# async {
axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
    .serve(
        app.into_make_service_with_connect_info::<SocketAddr>()
    )
    .await
    .expect("server failed");
# };
```

You can implement custom a [`Connected`] like so:

```rust
use axum::{
    extract::connect_info::{ConnectInfo, Connected},
    routing::get,
    Router,
};
use hyper::server::conn::AddrStream;

let app = Router::new().route("/", get(handler));

async fn handler(
    ConnectInfo(my_connect_info): ConnectInfo<MyConnectInfo>,
) -> String {
    format!("Hello {:?}", my_connect_info)
}

#[derive(Clone, Debug)]
struct MyConnectInfo {
    // ...
}

impl Connected<&AddrStream> for MyConnectInfo {
    fn connect_info(target: &AddrStream) -> Self {
        MyConnectInfo {
            // ...
        }
    }
}

# async {
axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
    .serve(
        app.into_make_service_with_connect_info::<MyConnectInfo>()
    )
    .await
    .expect("server failed");
# };
```

See the [unix domain socket example][uds] for an example of how to use
this to collect UDS connection info.

[`MakeService`]: tower::make::MakeService
[`Connected`]: crate::extract::connect_info::Connected
[`ConnectInfo`]: crate::extract::connect_info::ConnectInfo
[uds]: https://github.com/tokio-rs/axum/blob/main/examples/unix-domain-socket/src/main.rs
