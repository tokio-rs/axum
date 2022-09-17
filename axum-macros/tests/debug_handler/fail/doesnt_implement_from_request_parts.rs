use axum_macros::debug_handler;
use axum::http::Method;

#[debug_handler]
async fn handler(_: String, _: Method) {}

fn main() {}
