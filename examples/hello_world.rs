//! Run with
//!
//! ```not_rust
//! cargo run --example hello_world
//! ```

use axum::{handler::get, response::Html, route, routing::RoutingDsl};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "hello_world=debug")
    }
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = route("/", get(handler));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}
