//! Run with
//!
//! ```not_rust
//! cargo run --example tracing_aka_logging
//! ```

use axum::prelude::*;
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "tracing_aka_logging=debug,tower_http=debug")
    }
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = route("/", get(handler))
        // `TraceLayer` is provided by tower-http so you have to add that as a dependency.
        // It provides good defaults but is also very customizable.
        // See https://docs.rs/tower-http/0.1.1/tower_http/trace/index.html for more details.
        .layer(TraceLayer::new_for_http());

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> response::Html<&'static str> {
    response::Html("<h1>Hello, World!</h1>")
}
