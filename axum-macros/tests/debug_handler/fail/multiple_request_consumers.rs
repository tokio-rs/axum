use axum::{
    body::Bytes,
    http::{Method, Uri},
    Json,
};
use axum_macros::debug_handler;

#[debug_handler]
async fn one(_: Json<()>, _: String, _: Uri) {}

#[debug_handler]
async fn two(_: Json<()>, _: Method, _: Bytes, _: Uri, _: String) {}

fn main() {}
