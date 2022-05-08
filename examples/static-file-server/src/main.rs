//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-static-file-server
//! ```

use axum::{routing::get, Router};
use std::{convert::Infallible, io, net::SocketAddr};
use tower::{make::Shared, ServiceExt};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "example_static_file_server=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // `SpaRouter` is the easiest way to serve assets at a nested route like `/assets`
    // let app = Router::new()
    //     .route("/foo", get(|| async { "Hi from /foo" }))
    //     .merge(axum_extra::routing::SpaRouter::new("/assets", "."));

    // for serving assets directly at the root you can use `tower_http::services::ServeDir`
    // with a `Router` as the fallback
    let app = ServeDir::new(".")
        .fallback(
            Router::new()
                .route("/foo", get(|| async { "Hi from /foo" }))
                // `ServeDir::fallback` requires the error type to be `io::Error`
                .map_err(infallible_to_io_err),
        )
        // also call the fallback if the request isn't `GET` or `HEAD`
        .call_fallback_on_method_not_allowed(true);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(Shared::new(app))
        .await
        .unwrap();
}

fn infallible_to_io_err(err: Infallible) -> io::Error {
    match err {}
}
