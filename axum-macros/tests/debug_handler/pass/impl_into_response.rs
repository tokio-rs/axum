use axum_macros::debug_handler;
use axum::response::IntoResponse;

#[debug_handler]
async fn handler() -> impl IntoResponse {
    "hi!"
}

struct A;

impl A {
    #[debug_handler]
    async fn handler() -> impl IntoResponse {
        "hi!"
    }
}

fn main() {}
