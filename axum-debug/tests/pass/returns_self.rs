use axum::{
    body::BoxBody,
    response::{IntoResponse, Response},
};
use axum_debug::debug_handler;

struct A;

impl A {
    #[debug_handler]
    async fn handler() -> Self {
        A
    }
}

impl IntoResponse for A {
    fn into_response(self) -> Response {
        todo!()
    }
}

fn main() {}
