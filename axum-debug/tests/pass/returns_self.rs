use axum::{
    body::{Bytes, Full},
    http::Response,
    response::IntoResponse,
};
use axum_debug::debug_handler;
use std::convert::Infallible;

struct A;

impl A {
    #[debug_handler]
    async fn handler() -> Self {
        A
    }
}

impl IntoResponse for A {
    type Body = Full<Bytes>;
    type BodyError = Infallible;

    fn into_response(self) -> Response<Self::Body> {
        todo!()
    }
}

fn main() {}
