use axum_macros::debug_handler;
use axum::{Json, http::Uri};

#[debug_handler]
async fn one(_: Json<()>, _: Uri) {}

#[debug_handler]
async fn two(_: String, _: Uri) {}

fn main() {}
