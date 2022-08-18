use axum_macros::debug_handler;

#[debug_handler(body = BoxBody, body = BoxBody)]
async fn handler() {}

#[debug_handler(state = (), state = ())]
async fn handler_2() {}

fn main() {}
