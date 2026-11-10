use axum_macros::TypedPath;
use serde::Deserialize;

#[derive(TypedPath, Deserialize)]
#[typed_path("/users/{user-id}")]
struct Named {
    user_id: String,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/users/{123}")]
struct Tuple(String);

#[derive(TypedPath, Deserialize)]
#[typed_path("/files/{*file-name}")]
struct Wildcard(String);

#[derive(TypedPath, Deserialize)]
#[typed_path("/users/{user_id }")]
struct Whitespace(String);

fn main() {}
