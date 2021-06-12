use http::StatusCode;
use std::net::SocketAddr;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tower_web::{prelude::*, service::ServiceExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = tower_web::routing::nest(
        "/static",
        tower_web::service::get(ServeDir::new(".").handle_error(|error: std::io::Error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Unhandled interal error: {}", error),
            )
        })),
    )
    .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    app.serve(&addr).await.unwrap();
}
