use axum::{routing::post, Json, Router};
use serde_json::Value;
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, decompression::RequestDecompressionLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "compression=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app: Router = Router::new().route("/", post(root)).layer(
        ServiceBuilder::new()
            .layer(RequestDecompressionLayer::new())
            .layer(CompressionLayer::new()),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn root(Json(value): Json<Value>) -> Json<Value> {
    Json(value)
}
