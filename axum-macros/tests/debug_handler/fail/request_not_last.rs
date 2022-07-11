use axum::{extract::Extension, body::Body, http::Request};
use axum_macros::debug_handler;

#[debug_handler]
async fn handler(_: Request<Body>, _: Extension<String>) {}

fn main() {}
