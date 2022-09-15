use axum::{
    async_trait,
    extract::{FromRequest, RequestParts},
    http::StatusCode,
    response::IntoResponse,
};
use axum_macros::debug_handler;

struct ExampleExtract<T, U> {
    t: T,
    u: U,
}

impl<T, U> Default for ExampleExtract<T, U>
where
    T: Default,
    U: Default,
{
    fn default() -> Self {
        Self {
            t: Default::default(),
            u: Default::default(),
        }
    }
}

#[async_trait]
impl<B, T, U> FromRequest<B> for ExampleExtract<T, U>
where
    B: Send,
    T: Default,
    U: Default,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request(_req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        Ok(Default::default())
    }
}

#[debug_handler(with(T = String, T = u64; U = i32, U = i16))]
async fn handler<T, U>(_foo: ExampleExtract<T, U>) -> impl IntoResponse
where
    T: std::fmt::Display,
{
    "hi!"
}

fn main() {}
