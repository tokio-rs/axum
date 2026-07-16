use axum_macros::TypedPath;
use serde::Deserialize;

#[derive(TypedPath, Deserialize)]
#[typed_path("/files/{*rest}.txt")]
struct MyPath {
    rest: String,
}

fn main() {}
