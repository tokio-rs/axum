//! Example of application using spoofable extractors
//!
//! Run with
//!
//! ```not_rust
//! cargo run -p spoofable-scheme
//! ```
//!
//! Test with curl:
//!
//! ```not_rust
//! curl -i http://localhost:3000/ -H "X-Forwarded-Proto: http"
//! ```

use axum::{routing::get, Router};
use axum_extra::extract::{Scheme, Spoofable};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // build our application with some routes
    let app = Router::new().route("/", get(f));

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn f(Spoofable(Scheme(scheme)): Spoofable<Scheme>) -> String {
    scheme
}
