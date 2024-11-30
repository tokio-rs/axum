use assert_json_diff::assert_json_eq;
use axum::{
    body::{Body, Bytes},
    response::Response,
};
use brotli::enc::BrotliEncoderParams;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use http::{header, StatusCode};
use serde_json::{json, Value};
use std::io::{Read, Write};
use tower::ServiceExt;

use super::*;

#[tokio::test]
async fn handle_uncompressed_request_bodies() {
    // Given

    let body = json();

    let compressed_request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(json_body(&body))
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
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_ENCODING, "gzip")
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
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_ENCODING, "br")
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
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_ENCODING, "zstd")
        .body(Body::from(body))
        .unwrap();

    // When

    let response = app().oneshot(compressed_request).await.unwrap();

    // Then

    assert_eq!(response.status(), StatusCode::OK);
    assert_json_eq!(json_from_response(response).await, json());
}

#[tokio::test]
async fn do_not_compress_response_bodies() {
    // Given
    let request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(json_body(&json()))
        .unwrap();

    // When

    let response = app().oneshot(request).await.unwrap();

    // Then

    assert_eq!(response.status(), StatusCode::OK);
    assert_json_eq!(json_from_response(response).await, json());
}

#[tokio::test]
async fn compress_response_bodies_with_gzip() {
    // Given
    let request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT_ENCODING, "gzip")
        .body(json_body(&json()))
        .unwrap();

    // When

    let response = app().oneshot(request).await.unwrap();

    // Then

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = byte_from_response(response).await;
    let mut decoder = GzDecoder::new(response_body.as_ref());
    let mut decompress_body = String::new();
    decoder.read_to_string(&mut decompress_body).unwrap();
    assert_json_eq!(
        serde_json::from_str::<serde_json::Value>(&decompress_body).unwrap(),
        json()
    );
}

#[tokio::test]
async fn compress_response_bodies_with_br() {
    // Given
    let request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT_ENCODING, "br")
        .body(json_body(&json()))
        .unwrap();

    // When

    let response = app().oneshot(request).await.unwrap();

    // Then

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = byte_from_response(response).await;
    let mut decompress_body = Vec::new();
    brotli::BrotliDecompress(&mut response_body.as_ref(), &mut decompress_body).unwrap();
    assert_json_eq!(
        serde_json::from_slice::<serde_json::Value>(&decompress_body).unwrap(),
        json()
    );
}

#[tokio::test]
async fn compress_response_bodies_with_zstd() {
    // Given
    let request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT_ENCODING, "zstd")
        .body(json_body(&json()))
        .unwrap();

    // When

    let response = app().oneshot(request).await.unwrap();

    // Then

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = byte_from_response(response).await;
    let decompress_body = zstd::stream::decode_all(std::io::Cursor::new(response_body)).unwrap();
    assert_json_eq!(
        serde_json::from_slice::<serde_json::Value>(&decompress_body).unwrap(),
        json()
    );
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

fn json_body(input: &Value) -> Body {
    Body::from(serde_json::to_vec(&input).unwrap())
}

async fn json_from_response(response: Response) -> Value {
    let body = byte_from_response(response).await;
    body_as_json(body)
}

async fn byte_from_response(response: Response) -> Bytes {
    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
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
