use axum_macros::debug_handler;

#[debug_handler]
async fn handler(_one: String, _two: String, _three: String) {}

fn main() {}
