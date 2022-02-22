//! Run with
//!
//! ```not_rust
//! cargo run -p example-routing-match-http-method-for-any-path
//! # listening on 3000
//!
//! curl -v -X OPTIONS http://localhost:3000/some_path
//! # OPTIONS /some_path matched!
//!
//! curl -v -X GET http://localhost:3000/some_path
//! # GET /some_path matched!
//!
//! curl -v -X OPTIONS http://localhost:3000/another_path
//! # OPTIONS /another_path matched!
//!
//! curl -v -X GET http://localhost:3000/another_path
//! # GET /another_path did not match anything!
//! ```

use axum::{
    body::{Body, HttpBody},
    http::{Method, Request},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = Router::new()
        // matches "OPTIONS" method for any path
        .layer(middleware::from_fn(intercept_options_method))
        // matches "GET" method for "/some_path" only
        .route("/some_path", get(get_some_path_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn intercept_options_method<B>(req: Request<B>, next: Next<B>) -> impl IntoResponse {
    if req.method() == Method::OPTIONS {
        Ok(Response::builder()
            .header("x-matched-from", req.uri().to_string())
            .status(200)
            .body(Body::from("OPTIONS matched!").boxed_unsync())
            .unwrap())
    } else {
        Ok(next.run(req).await)
    }
}

async fn get_some_path_handler<B>(req: Request<B>) -> impl IntoResponse {
    Response::builder()
        .header("x-matched-from", req.uri().to_string())
        .status(200)
        .body(Body::from("GET /some_path matched!").boxed_unsync())
        .unwrap()
}
