//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-hello-world
//! ```

use axum::{response::Html, routing::get, Router};
use std::net::SocketAddr;
use tower_http::compression::{predicate::SizeAbove, CompressionLayer};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .nest("/", Router::new().route("/", get(|| async { "Hello, World!" })))
        .layer(CompressionLayer::new().compress_when(SizeAbove::new(0)));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}
