use axum_macros::TypedPath;
use serde::Deserialize;

#[derive(TypedPath, Deserialize)]
#[typed_path("/user-{first}-{last}")]
struct MyPath {
    first: String,
    last: String,
}

fn main() {}
