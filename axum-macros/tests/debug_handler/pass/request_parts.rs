use axum::http::{request::Parts, Method};
use axum_macros::debug_handler;

#[debug_handler]
async fn only_parts(_: Parts) {}

#[debug_handler]
async fn parts_first(_: Parts, _: Method) {}

#[debug_handler]
async fn parts_and_body(_: Parts, _: String) {}

fn main() {}
