//! Run with
//!
//! ```not_rust
//! cargo run -p example-reqwest-response
//! ```

use axum::{
    body::{Body, Bytes},
    extract::{Request, State},
    response::Response,
    routing::get,
    Router,
};
use reqwest::Client;
use std::{convert::Infallible, time::Duration};
use sync_wrapper::SyncStream;
use tokio_stream::StreamExt;
use tower_http::trace::TraceLayer;
use tracing::Span;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_reqwest_response=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let client = Client::new();

    let app = Router::new()
        .route("/", get(proxy_via_reqwest))
        .route("/stream", get(stream_some_data))
        // Add some logging so we can see the streams going through
        .layer(TraceLayer::new_for_http().on_body_chunk(
            |chunk: &Bytes, _latency: Duration, _span: &Span| {
                tracing::debug!("streaming {} bytes", chunk.len());
            },
        ))
        .with_state(client);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn proxy_via_reqwest(
    State(client): State<Client>,
    mut req: Request<Body>,
) -> Response<reqwest::Body> {
    *req.uri_mut() = "http://127.0.0.1:3000/stream".parse().unwrap();
    let req = req.map(|body| reqwest::Body::wrap_stream(SyncStream::new(body.into_data_stream())));
    let req = reqwest::Request::try_from(req).unwrap();
    client.execute(req).await.unwrap().into()
}

async fn stream_some_data() -> Body {
    let stream = tokio_stream::iter(0..5)
        .throttle(Duration::from_secs(1))
        .map(|n| n.to_string())
        .map(Ok::<_, Infallible>);
    Body::from_stream(stream)
}
