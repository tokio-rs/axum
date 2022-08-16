use axum::extract::Path;
use axum::response::IntoResponse;
use axum_macros::debug_handler;

#[debug_handler(with(T = String, T = u64))]
async fn handler<T>(_extract_t: Path<T>) -> impl IntoResponse
where
    T: std::fmt::Display,
{
    "hi!"
}

fn main() {}
