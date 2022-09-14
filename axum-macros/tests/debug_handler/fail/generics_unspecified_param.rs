use axum::extract::{Path, Json};
use axum::response::IntoResponse;
use axum_macros::debug_handler;

#[debug_handler(with(T = String, T = u64))]
async fn handler<T, U>(_extract_t: Path<T>, _u: Json<U>) -> impl IntoResponse
where
    T: std::fmt::Display,
{
    "hi!"
}

fn main() {}
