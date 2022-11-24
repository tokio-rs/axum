//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-simple-router-wasm
//! ```
//!
//! This example shows what using axum in a wasm context might look like. This example should
//! always compile with `--target wasm32-unknown-unknown`.
//!
//! [`mio`](https://docs.rs/mio/latest/mio/index.html), tokio's IO layer, does not support the
//! `wasm32-unknown-unknown` target which is why this crate requires `default-features = false`
//! for axum.
//!
//! Most serverless runtimes expect an exported function that takes in a single request and returns
//! a single response, much like axum's `Handler` trait. In this example, the handler function is
//! `app` with `main` acting as the serverless runtime which originally receives the request and
//! calls the app function.
//!
//! We can use axum's routing, extractors, tower services, and everything else to implement
//! our serverless function, even though we are running axum in a wasm context.

use axum::{
    response::{Html, Response},
    routing::get,
    Router,
};
use futures_executor::block_on;
use http::Request;
use tower_service::Service;

fn main() {
    let request: Request<String> = Request::builder()
        .uri("https://serverless.example/api/")
        .body("Some Body Data".into())
        .unwrap();

    let response: Response = block_on(app(request));
    assert_eq!(200, response.status());
}

#[allow(clippy::let_and_return)]
async fn app(request: Request<String>) -> Response {
    let mut router = Router::new().route("/api/", get(index));
    let response = router.call(request).await.unwrap();
    response
}

async fn index() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}
