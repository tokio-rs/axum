#![allow(warnings)]

use axum::{
    extract::rejection::JsonRejection,
    response::{IntoResponse, Response},
    routing::get,
    Router, http::Method,
};
use axum_macros::FromRequest;
use serde::Deserialize;

fn main() {
    let _: Router = Router::new().route("/", get(handler));
}

async fn handler(_: MyJson) {}

#[derive(FromRequest)]
struct MyJson {
    also_body: Method,
    body: String,
}
