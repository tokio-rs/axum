use axum::{extract::Extension, body::Body, http::Request};
use axum_macros::debug_handler;

#[debug_handler]
async fn handler(_: Extension<String>, _: Request<Body>) {}

fn main() {}
