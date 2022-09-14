use axum_macros::debug_handler;

#[debug_handler(with(T = String, T = u64; U = i64; T = u32))]
async fn handler() {}

fn main() {}
