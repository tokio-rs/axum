//! Run with
//!
//! ```not_rust
//! cargo run -p example-handle-head-request
//! ```

use axum::response::{IntoResponse, Response};
use axum::{http, routing::get, Router};

fn app() -> Router {
    Router::new().route("/get-head", get(get_head_handler))
}

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app()).await.unwrap();
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
    use http_body_util::BodyExt;
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

        let body = response.collect().await.unwrap().to_bytes();
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

        let body = response.collect().await.unwrap().to_bytes();
        assert!(body.is_empty());
    }
}
