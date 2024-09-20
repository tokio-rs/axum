use axum_macros::debug_handler;

#[debug_handler]
async fn handler(_foo: bool) {}

fn main() {}
