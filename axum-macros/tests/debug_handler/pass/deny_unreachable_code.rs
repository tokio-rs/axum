#![deny(unreachable_code)]

use axum::extract::Path;

#[axum_macros::debug_handler]
async fn handler(Path(_): Path<String>) {}

fn main() {}
