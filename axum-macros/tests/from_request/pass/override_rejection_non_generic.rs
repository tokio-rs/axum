use axum::{
    extract::rejection::JsonRejection,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_macros::FromRequest;
use serde::Deserialize;
use std::collections::HashMap;

fn main() {
    let _: Router = Router::new().route("/", get(handler).post(handler_result));
}

async fn handler(_: MyJson) {}

async fn handler_result(_: Result<MyJson, MyJsonRejection>) {}

#[derive(FromRequest, Deserialize)]
#[from_request(via(axum::extract::Json), rejection(MyJsonRejection))]
#[serde(transparent)]
struct MyJson(HashMap<String, String>);

struct MyJsonRejection {}

impl From<JsonRejection> for MyJsonRejection {
    fn from(_: JsonRejection) -> Self {
        todo!()
    }
}

impl IntoResponse for MyJsonRejection {
    fn into_response(self) -> Response {
        todo!()
    }
}
