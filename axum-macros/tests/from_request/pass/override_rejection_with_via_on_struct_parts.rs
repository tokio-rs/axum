use axum::{
    extract::rejection::QueryRejection,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_macros::FromRequestParts;
use serde::Deserialize;

fn main() {
    let _: Router = Router::new().route("/", get(handler).post(handler_result));
}

#[derive(Deserialize)]
struct Payload {}

async fn handler(_: MyQuery<Payload>) {}

async fn handler_result(_: Result<MyQuery<Payload>, MyQueryRejection>) {}

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(MyQueryRejection))]
struct MyQuery<T>(T);

struct MyQueryRejection {}

impl From<QueryRejection> for MyQueryRejection {
    fn from(_: QueryRejection) -> Self {
        todo!()
    }
}

impl IntoResponse for MyQueryRejection {
    fn into_response(self) -> Response {
        todo!()
    }
}
