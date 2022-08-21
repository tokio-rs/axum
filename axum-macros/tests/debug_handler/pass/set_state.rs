use axum_macros::debug_handler;
use axum::extract::{FromRef, FromRequest};
use axum::async_trait;
use axum::http::Request;

#[debug_handler(state = AppState)]
async fn handler(_: A) {}

#[derive(Clone)]
struct AppState;

struct A;

#[async_trait]
impl<S, B> FromRequest<S, B> for A
where
    B: Send + 'static,
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = ();

    async fn from_request(_req: Request<B>, _state: &S) -> Result<Self, Self::Rejection> {
        unimplemented!()
    }
}

fn main() {}
