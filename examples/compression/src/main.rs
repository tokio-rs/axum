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

    let app: Router = app();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

fn app() -> Router {
    Router::new().route("/", post(root)).layer(
        ServiceBuilder::new()
            .layer(RequestDecompressionLayer::new())
            .layer(CompressionLayer::new()),
    )
}

async fn root(Json(value): Json<Value>) -> Json<Value> {
    Json(value)
}

#[cfg(test)]
mod tests {
    use assert_json_diff::assert_json_eq;
    use axum::{
        body::{Body, Bytes},
        response::Response,
    };
    use brotli::enc::BrotliEncoderParams;
    use flate2::{write::GzEncoder, Compression};
    use http::StatusCode;
    use serde_json::{json, Value};
    use std::io::Write;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn handle_uncompressed_request_bodies() {
        // Given

        let body = serde_json::to_vec(&json()).unwrap();

        let compressed_request = http::Request::post("/")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        // When

        let response = app().oneshot(compressed_request).await.unwrap();

        // Then

        assert_eq!(response.status(), StatusCode::OK);
        assert_json_eq!(json_from_response(response).await, json());
    }

    #[tokio::test]
    async fn decompress_gzip_request_bodies() {
        // Given

        let body = compress_gzip(&json());

        let compressed_request = http::Request::post("/")
            .header(http::header::CONTENT_TYPE, "application/json")
            .header("Content-Encoding", "gzip")
            .body(Body::from(body))
            .unwrap();

        // When

        let response = app().oneshot(compressed_request).await.unwrap();

        // Then

        assert_eq!(response.status(), StatusCode::OK);
        assert_json_eq!(json_from_response(response).await, json());
    }

    #[tokio::test]
    async fn decompress_br_request_bodies() {
        // Given

        let body = compress_br(&json());

        let compressed_request = http::Request::post("/")
            .header(http::header::CONTENT_TYPE, "application/json")
            .header("Content-Encoding", "br")
            .body(Body::from(body))
            .unwrap();

        // When

        let response = app().oneshot(compressed_request).await.unwrap();

        // Then

        assert_eq!(response.status(), StatusCode::OK);
        assert_json_eq!(json_from_response(response).await, json());
    }

    #[tokio::test]
    async fn decompress_zstd_request_bodies() {
        // Given

        let body = compress_zstd(&json());

        let compressed_request = http::Request::post("/")
            .header(http::header::CONTENT_TYPE, "application/json")
            .header("Content-Encoding", "zstd")
            .body(Body::from(body))
            .unwrap();

        // When

        let response = app().oneshot(compressed_request).await.unwrap();

        // Then

        assert_eq!(response.status(), StatusCode::OK);
        assert_json_eq!(json_from_response(response).await, json());
    }

    fn json() -> Value {
        json!({
          "name": "foo",
          "mainProduct": {
            "typeId": "product",
            "id": "p1"
          },
        })
    }

    async fn json_from_response(response: Response) -> Value {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        body_as_json(body)
    }

    fn body_as_json(body: Bytes) -> Value {
        serde_json::from_slice(body.as_ref()).unwrap()
    }

    fn compress_gzip(json: &Value) -> Vec<u8> {
        let request_body = serde_json::to_vec(&json).unwrap();

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&request_body).unwrap();
        encoder.finish().unwrap()
    }

    fn compress_br(json: &Value) -> Vec<u8> {
        let request_body = serde_json::to_vec(&json).unwrap();
        let mut result = Vec::new();

        let params = BrotliEncoderParams::default();
        let _ = brotli::enc::BrotliCompress(&mut &request_body[..], &mut result, &params).unwrap();

        result
    }

    fn compress_zstd(json: &Value) -> Vec<u8> {
        let request_body = serde_json::to_vec(&json).unwrap();
        zstd::stream::encode_all(std::io::Cursor::new(request_body), 4).unwrap()
    }
}
