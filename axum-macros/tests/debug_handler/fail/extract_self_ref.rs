use axum::{
    async_trait,
    extract::{FromRequest, RequestParts},
};
use axum_macros::debug_handler;

struct A;

#[async_trait]
impl<R, B> FromRequest<R, B> for A
where
    B: Send + 'static,
{
    type Rejection = ();

    async fn from_request(_req: &mut RequestParts<R, B>) -> Result<Self, Self::Rejection> {
        unimplemented!()
    }
}

impl A {
    #[debug_handler]
    async fn handler(&self) {}
}

fn main() {}
