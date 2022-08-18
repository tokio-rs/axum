mod with_rejection;

use std::net::SocketAddr;
use axum::{Server, Router, routing::get};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};



#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "error_handling=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Build our application with some routes
    let app = Router::new()
        .route("/withRejection", get(with_rejection::handler));

    // Run our application
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}