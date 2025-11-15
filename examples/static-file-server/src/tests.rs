use super::{using_serve_dir, using_serve_dir_with_assets_fallback, Router};
use axum::http::StatusCode;
use axum::{body::Body, http::Request};
use headers::ContentType;
use http_body_util::BodyExt;
use tower::ServiceExt;

const INDEX_HTML_CONTENT: &str = include_str!("../assets/index.html");
const SCRIPT_JS_CONTENT: &str = include_str!("../assets/script.js");

async fn get_page(app: Router, path: &str) -> (StatusCode, Option<ContentType>, String) {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();

    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .map(|header| header.to_str().unwrap().parse::<ContentType>().unwrap());

    let body = response.into_body();
    let bytes = body.collect().await.unwrap().to_bytes();
    let html = String::from_utf8(bytes.to_vec()).unwrap();

    (status, content_type, html)
}

async fn check(
    app: Router,
    path: &str,
    status: StatusCode,
    content_type: Option<ContentType>,
    content: &str,
) {
    let (actual_status, actual_content_type, actual_content) = get_page(app, path).await;
    assert_eq!(status, actual_status);
    assert_eq!(content_type, actual_content_type);
    assert_eq!(content, actual_content);
}

#[tokio::test]
async fn test_using_serve_dir() {
    let app = using_serve_dir;
    check(
        app(),
        "/assets/index.html",
        StatusCode::OK,
        Some(ContentType::html()),
        INDEX_HTML_CONTENT,
    )
    .await;
    check(
        app(),
        "/assets/script.js",
        StatusCode::OK,
        Some(ContentType::from(mime::TEXT_JAVASCRIPT)),
        SCRIPT_JS_CONTENT,
    )
    .await;
    check(
        app(),
        "/assets/",
        StatusCode::OK,
        Some(ContentType::html()),
        INDEX_HTML_CONTENT,
    )
    .await;
    check(app(), "/assets/other.html", StatusCode::NOT_FOUND, None, "").await;
}

#[tokio::test]
async fn test_using_serve_dir_with_assets_fallback() {
    let app = using_serve_dir_with_assets_fallback;
    check(
        app(),
        "/assets/index.html",
        StatusCode::OK,
        Some(ContentType::html()),
        INDEX_HTML_CONTENT,
    )
    .await;
    check(
        app(),
        "/assets/script.js",
        StatusCode::OK,
        Some(ContentType::from(mime::TEXT_JAVASCRIPT)),
        SCRIPT_JS_CONTENT,
    )
    .await;
    check(
        app(),
        "/assets/",
        StatusCode::OK,
        Some(ContentType::html()),
        INDEX_HTML_CONTENT,
    )
    .await;

    check(
        app(),
        "/foo",
        StatusCode::OK,
        Some(ContentType::text_utf8()),
        "Hi from /foo",
    )
    .await;
}
