use axum::response::IntoResponse;
use axum_macros::debug_handler;

#[debug_handler]
async fn handler() -> impl IntoResponse {
    "hi!"
}

fn main() {}
