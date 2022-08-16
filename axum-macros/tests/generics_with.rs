use axum::extract::Path;
use axum::response::IntoResponse;
use axum_macros::debug_handler;

// #[debug_handler(with(T = String, T = u64; U = i8, U = i16; N = 0))]
#[debug_handler(with(T = String, T = u64))]
async fn handler<T>(extract_t: Path<T>) -> impl IntoResponse
// async fn handler<T>(extract_t: T) -> impl IntoResponse
where
    T: std::fmt::Display,
{
    "hi!"
}

fn main() {}
