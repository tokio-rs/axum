//! Run with
//!
//! ```not_rust
//! cargo run -p example-reqwest-response
//! ```

use std::{convert::Infallible, time::Duration};

use axum::http::StatusCode;
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{HeaderName, HeaderValue},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use reqwest::Client;
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

async fn proxy_via_reqwest(State(client): State<Client>) -> Response {
    let reqwest_response = match client.get("http://127.0.0.1:3000/stream").send().await {
        Ok(res) => res,
        Err(err) => {
            tracing::error!(%err, "request failed");
            return (StatusCode::BAD_REQUEST, Body::empty()).into_response();
        }
    };

    let mut response_builder = Response::builder().status(reqwest_response.status().as_u16());

    {
        let headers = response_builder.headers_mut().unwrap();

        reqwest_response
            .headers()
            .into_iter()
            .for_each(|(name, value)| {
                let name = HeaderName::from_bytes(name.as_ref()).unwrap();
                let value = HeaderValue::from_bytes(value.as_ref()).unwrap();
                headers.insert(name, value);
            });
    }

    response_builder
        .body(Body::from_stream(reqwest_response.bytes_stream()))
        // This unwrap is fine because the body is empty here
        .unwrap()
}

async fn stream_some_data() -> Body {
    let stream = tokio_stream::iter(0..5)
        .throttle(Duration::from_secs(1))
        .map(|n| n.to_string())
        .map(Ok::<_, Infallible>);
    Body::from_stream(stream)
}
