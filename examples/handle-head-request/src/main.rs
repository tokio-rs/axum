//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-handle-head-reqeust
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

// get routes will also be called for HEAD requests but will have the response body removed
// can handle implicit head method request by extract `http::Method` from request
async fn get_head_handler(method: http::Method) -> Response {
    let something = Some(Thing);

    // it would usually handle only special-case HEAD
    // if computing the body has some relevant compute cost
    if method == http::Method::HEAD {
        // it could do lightweight check in HEAD
        return if something.is_some() {
            ([("x-some-header", "header from HEAD")]).into_response()
        } else {
            (http::StatusCode::NOT_FOUND).into_response()
        };
    }

    // then do some computing task in GET
    if let Some(thing) = something {
        thing.do_some_computing_task();
    }

    ([("x-some-header", "header from GET")], "body from GET").into_response()
}

struct Thing;

impl Thing {
    fn do_some_computing_task(&self) {
        // TODO
    }
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
