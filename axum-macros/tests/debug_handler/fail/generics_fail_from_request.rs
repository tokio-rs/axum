use axum::extract::Path;
use axum::response::IntoResponse;
use axum_macros::debug_handler;

#[debug_handler(with(T = &'static str, T = Path<String>))]
async fn handler<T>(_foo: T) -> impl IntoResponse {
    "hi!"
}

fn main() {}
