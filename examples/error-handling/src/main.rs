mod custom_extractor;
mod derive_from_request;
mod with_rejection;

use axum::{routing::post, Router};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "error_handling=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Build our application with some routes
    let app = Router::new()
        .route("/withRejection", post(with_rejection::handler))
        .route("/customExtractor", post(custom_extractor::handler))
        .route("/deriveFromRequest", post(derive_from_request::handler));

    // Run our application
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
