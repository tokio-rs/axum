//! Run with
//!
//! ```not_rust
//! cargo run --example 404
//! ```

use axum::{
    body::{box_body, Body, BoxBody},
    prelude::*,
};
use http::{Response, StatusCode};
use std::net::SocketAddr;
use tower::util::MapResponseLayer;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = route("/", get(handler))
        // make sure this is added as the very last thing
        .layer(MapResponseLayer::new(map_404));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    hyper::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> response::Html<&'static str> {
    response::Html("<h1>Hello, World!</h1>")
}

fn map_404(response: Response<BoxBody>) -> Response<BoxBody> {
    if response.status() != StatusCode::NOT_FOUND {
        return response;
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(box_body(Body::from("nothing to see here")))
        .unwrap()
}
