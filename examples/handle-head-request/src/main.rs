//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-handle-head-request
//! ```

use axum::response::{IntoResponse, Response};
use axum::{http, routing::get, Router};
use std::net::SocketAddr;

fn app() -> Router {
    Router::new().route("/get-head", get(get_head_handler))
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    axum::Server::bind(&addr)
        .serve(app().into_make_service())
        .await
        .unwrap();
}

// GET routes will also be called for HEAD requests but will have the response body removed.
// You can handle the HEAD method explicitly by extracting `http::Method` from the request.
async fn get_head_handler(method: http::Method) -> Response {
    // it usually only makes sense to special-case HEAD
    // if computing the body has some relevant cost
    if method == http::Method::HEAD {
        return ([("x-some-header", "header from HEAD")]).into_response();
    }

    // then do some computing task in GET
    do_some_computing_task();

    ([("x-some-header", "header from GET")], "body from GET").into_response()
}

fn do_some_computing_task() {
    // TODO
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get() {
        let app = app();

        let response = app
            .oneshot(Request::get("/get-head").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-some-header"], "header from GET");

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], b"body from GET");
    }

    #[tokio::test]
    async fn test_implicit_head() {
        let app = app();

        let response = app
            .oneshot(Request::head("/get-head").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-some-header"], "header from HEAD");

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert!(body.is_empty());
    }
}
