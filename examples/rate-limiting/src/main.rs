//! Rate limiting example using `tower::limit::RateLimitLayer`.
//!
//! `RateLimit` does not implement `Clone`, so it must be wrapped with
//! `BufferLayer` to satisfy axum's `Clone` requirement. `HandleErrorLayer`
//! converts middleware errors (from `Buffer` / `RateLimit`) into HTTP
//! responses.
//!
//! Run with:
//!
//! ```not_rust
//! cargo run -p example-rate-limiting
//! ```
//!
//! Then try sending requests rapidly:
//!
//! ```not_rust
//! # Send several requests at once — excess requests get 503
//! for i in $(seq 1 8); do curl -sw '\n' http://127.0.0.1:3000/ & done; wait
//! ```

use axum::{
    error_handling::HandleErrorLayer, http::StatusCode, response::IntoResponse, routing::get,
    Router,
};
use std::time::Duration;
use tower::{buffer::BufferLayer, limit::RateLimitLayer, BoxError, ServiceBuilder};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/slow", get(slow_handler))
        // Apply a global rate limit: 5 requests per second.
        //
        // HandleErrorLayer converts tower errors into HTTP responses.
        // BufferLayer wraps the non-Clone RateLimit service so that axum
        // can clone it across tasks.
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .layer(BufferLayer::new(1024))
                .layer(RateLimitLayer::new(5, Duration::from_secs(1))),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await;
}

async fn slow_handler() -> &'static str {
    tokio::time::sleep(Duration::from_secs(1)).await;
    "This was slow!"
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    tracing::error!(%error, "unhandled middleware error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Something went wrong".to_string(),
    )
}
