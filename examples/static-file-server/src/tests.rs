use super::using_serve_dir_with_assets_fallback;
use axum::{body::Body, http::Request, http::StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

const INDEX_HTML_CONTENT: &str = include_str!("../assets/index.html");

// `/script.js` at the root is not an asset under `/assets`, so it falls through to the SPA
// fallback. The fallback serves `index.html` with a 404 status -- the path wasn't found, but
// the SPA handles routing client-side.
#[tokio::test]
async fn assets_not_served_at_root() {
    let app = using_serve_dir_with_assets_fallback();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/script.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(body_str, INDEX_HTML_CONTENT);
}
