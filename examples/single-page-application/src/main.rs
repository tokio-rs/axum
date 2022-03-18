//! Run with
//!
//! ```not_rust
//! cd examples/single-page-application/frontend/ && npm install && npm run build
//! cargo run -p example-single-page-application
//! ```

use axum::{http::StatusCode, routing::{get, get_service}, Router};
use axum::response::{Html, IntoResponse};
use std::net::SocketAddr;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "example_single_page_application=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // note that we must use `assets` here, if you like a different name,
    // you need set it in `vite.config.ts` via `build.assetsDir` (which default to `assets`)
    // see https://vitejs.dev/config/#build-assetsdir
    let app = Router::new()
        .route("/", get(index_html))
        .route("/api/auth", get(|| async { "API: auth" }))
        .nest(
            "/assets",
            get_service(ServeDir::new("examples/single-page-application/frontend/dist/assets")).handle_error(|error: std::io::Error| async move {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unhandled internal error: {}", error),
                )
            }),
        )
        .fallback(get(index_html))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

pub async fn index_html() -> impl IntoResponse {
    Html(include_str!("../frontend/dist/index.html"))
}
