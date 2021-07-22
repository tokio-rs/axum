use axum::{prelude::*, routing::nest, service::ServiceExt};
use http::StatusCode;
use std::net::SocketAddr;
use tower_http::{services::ServeDir, trace::TraceLayer};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = nest(
        "/static",
        axum::service::get(ServeDir::new(".").handle_error(|error: std::io::Error| {
            Ok::<_, std::convert::Infallible>((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Unhandled interal error: {}", error),
            ))
        })),
    )
    .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    hyper::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
