use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt; // for `collect`
use tower::ServiceExt; // for `call`, `oneshot`, and `ready`

#[tokio::test]
async fn hello_world() {
    let app = create_router();

    let response = app
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<h1>Hello, World!</h1>");
}

#[tokio::test]
async fn test_404() {
    let app = create_router();

    let response = app
        .oneshot(Request::get("/other").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"nothing to see here");
}
