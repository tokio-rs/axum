use axum::extract::{Extension, Request};
use axum_macros::debug_handler;

#[debug_handler]
async fn handler(_: Request, _: Extension<String>) {}

fn main() {}
