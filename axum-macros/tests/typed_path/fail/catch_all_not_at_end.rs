use axum_macros::TypedPath;
use serde::Deserialize;

#[derive(TypedPath, Deserialize)]
#[typed_path("/{*rest}/foo")]
struct MyPath {
    rest: String,
}

fn main() {}
