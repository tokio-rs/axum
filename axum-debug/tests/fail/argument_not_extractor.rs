use axum_debug::debug_handler;

#[debug_handler]
async fn handler(foo: bool) {}

fn main() {}
