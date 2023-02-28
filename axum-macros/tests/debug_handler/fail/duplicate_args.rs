use axum_macros::debug_handler;

#[debug_handler(state = (), state = ())]
async fn handler() {}

fn main() {}
