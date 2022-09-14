use axum_macros::debug_handler;

#[debug_handler(with(T = String, U = u64))]
async fn handler() {}

fn main() {}
