use axum::response::{IntoResponseParts, ResponseParts};
use axum_macros::debug_handler;

struct A;

impl A {
    #[debug_handler]
    async fn handler() -> Self {
        A
    }
}

impl IntoResponseParts for A {
    fn into_response_parts(self, _res: &mut ResponseParts) {
        todo!()
    }
}

fn main() {}
