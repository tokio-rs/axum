use axum::{
    async_trait,
    extract::FromRequest,
    http::Request,
};
use axum_macros::debug_handler;

struct A;

#[async_trait]
impl<S, B> FromRequest<S, B> for A
where
    B: Send + 'static,
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request(_req: Request<B>, _state: &S) -> Result<Self, Self::Rejection> {
        unimplemented!()
    }
}

impl A {
    #[debug_handler]
    async fn handler(&mut self) {}
}

fn main() {}
