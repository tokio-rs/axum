use axum_macros::debug_handler;
use axum::extract::{Request, FromRef, FromRequest};
use axum::async_trait;

#[debug_handler(state = AppState)]
async fn handler(_: A) {}

#[derive(Clone)]
struct AppState;

struct A;

#[async_trait]
impl<S> FromRequest<S> for A
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = ();

    async fn from_request(_req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        unimplemented!()
    }
}

fn main() {}
