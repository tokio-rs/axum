use axum_debug::debug_handler;

#[debug_handler(foo)]
async fn handler() {}

fn main() {}
