use axum::{http::Uri, Json};
use axum_macros::debug_handler;

#[debug_handler]
async fn one(_: Json<()>, _: Uri) {}

#[debug_handler]
async fn two(_: String, _: Uri) {}

fn main() {}
