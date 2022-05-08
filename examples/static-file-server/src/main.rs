//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-static-file-server
//! ```

use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, get_service},
    Router,
};
use std::{io, net::SocketAddr};
use tower_http::{services::ServeDir, trace::TraceLayer};
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
    //     .merge(axum_extra::routing::SpaRouter::new("/assets", "."))
    //     .layer(TraceLayer::new_for_http());

    // for serving assets directly at the root you can use `tower_http::services::ServeDir`
    // as the fallback to a `Router`
    let app: _ = Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .fallback(get_service(ServeDir::new(".")).handle_error(handle_error))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}
