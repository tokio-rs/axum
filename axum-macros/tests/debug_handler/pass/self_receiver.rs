use axum::extract::{FromRequest, Request};
use axum_macros::debug_handler;

struct A;

impl<S> FromRequest<S> for A
where
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request(_req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        unimplemented!()
    }
}

impl<S> FromRequest<S> for Box<A>
where
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request(_req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        unimplemented!()
    }
}

impl A {
    #[debug_handler]
    async fn handler(self) {}

    #[debug_handler]
    async fn handler_with_qualified_self(self: Box<Self>) {}
}

fn main() {}
