use super::*;
use axum::http::StatusCode as SC;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use tower::ServiceExt;

const INDEX_HTML: &str = include_str!("../assets/index.html");
const SCRIPT_JS: &str = include_str!("../assets/script.js");

const JS: &str = "text/javascript";
const HTML: &str = "text/html";

async fn get_page(app: Router, path: &str) -> (StatusCode, String, String) {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();

    let status = response.status();
    let content_type = match response.headers().get("content-type") {
        Some(content_type) => content_type.to_str().unwrap().to_owned(),
        None => String::new(),
    };

    let body = response.into_body();
    let bytes = body.collect().await.unwrap().to_bytes();
    let html = String::from_utf8(bytes.to_vec()).unwrap();

    (status, content_type, html)
}

async fn check(app: Router, path: &str, status: StatusCode, content_type: &str, content: &str) {
    let (actual_status, actual_content_type, actual_content) = get_page(app, path).await;
    assert_eq!(status, actual_status);
    assert_eq!(content_type, actual_content_type);
    assert_eq!(content, actual_content);
}

#[tokio::test]
async fn test_using_serve_dir() {
    let app = using_serve_dir;
    check(app(), "/assets/index.html", SC::OK, HTML, INDEX_HTML).await;
    check(app(), "/assets/script.js", SC::OK, JS, SCRIPT_JS).await;
    check(app(), "/assets/", SC::OK, HTML, INDEX_HTML).await;

    check(app(), "/assets/other.html", SC::NOT_FOUND, "", "").await;
}

#[tokio::test]
async fn test_using_serve_dir_with_assets_fallback() {
    let app = using_serve_dir_with_assets_fallback;
    check(app(), "/assets/index.html", SC::OK, HTML, INDEX_HTML).await;
    check(app(), "/assets/script.js", SC::OK, JS, SCRIPT_JS).await;
    check(app(), "/assets/", SC::OK, HTML, INDEX_HTML).await;

    check(
        app(),
        "/foo",
        SC::OK,
        "text/plain; charset=utf-8",
        "Hi from /foo",
    )
    .await;
}
