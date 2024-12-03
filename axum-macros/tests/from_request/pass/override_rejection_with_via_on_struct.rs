use axum::{
    extract::rejection::JsonRejection,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_macros::FromRequest;
use serde::Deserialize;

fn main() {
    let _: Router = Router::new().route("/", get(handler).post(handler_result));
}

#[derive(Deserialize)]
struct Payload {}

async fn handler(_: MyJson<Payload>) {}

async fn handler_result(_: Result<MyJson<Payload>, MyJsonRejection>) {}

#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(MyJsonRejection))]
struct MyJson<T>(T);

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
