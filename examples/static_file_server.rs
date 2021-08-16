//! Run with
//!
//! ```not_rust
//! cargo run --example static_file_server
//! ```

use axum::routing::{nest, RoutingDsl};
use http::StatusCode;
use std::net::SocketAddr;
use tower_http::{services::ServeDir, trace::TraceLayer};

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "static_file_server=debug,tower_http=debug")
    }
    tracing_subscriber::fmt::init();

    let app = nest(
        "/static",
        axum::service::get(ServeDir::new(".")).handle_error(|error: std::io::Error| {
            Ok::<_, std::convert::Infallible>((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Unhandled internal error: {}", error),
            ))
        }),
    )
    .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
