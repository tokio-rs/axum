use axum::http::{Method, Uri};
use axum_macros::debug_handler;

#[debug_handler]
async fn handler(_one: Method, _two: Uri, _three: String) {}

fn main() {}
