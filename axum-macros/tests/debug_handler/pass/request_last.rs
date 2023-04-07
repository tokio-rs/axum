use axum::extract::{Extension, Request};
use axum_macros::debug_handler;

#[debug_handler]
async fn handler(_: Extension<String>, _: Request) {}

fn main() {}
