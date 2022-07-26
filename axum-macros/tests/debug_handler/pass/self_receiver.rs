use axum::{
    async_trait,
    extract::{FromRequest, RequestParts},
};
use axum_macros::debug_handler;

struct A;

#[async_trait]
impl<B, S> FromRequest<B, S> for A
where
    B: Send,
    S: Send,
{
    type Rejection = ();

    async fn from_request(_req: &mut RequestParts<B, S>) -> Result<Self, Self::Rejection> {
        unimplemented!()
    }
}

impl A {
    #[debug_handler]
    async fn handler(self) {}
}

fn main() {}
