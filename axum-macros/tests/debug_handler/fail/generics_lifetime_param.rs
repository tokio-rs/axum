use axum::extract::Path;
use axum_macros::debug_handler;

#[debug_handler(with(T = String, T = u64))]
async fn handler<'a, T>(_extract_t: Path<T>) -> &'a str {
    "hi!"
}

fn main() {}
