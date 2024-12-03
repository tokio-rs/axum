use axum::{
    extract::rejection::QueryRejection,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_macros::FromRequestParts;
use serde::Deserialize;
use std::collections::HashMap;

fn main() {
    let _: Router = Router::new().route("/", get(handler).post(handler_result));
}

async fn handler(_: MyQuery) {}

async fn handler_result(_: Result<MyQuery, MyQueryRejection>) {}

#[derive(FromRequestParts, Deserialize)]
#[from_request(via(axum::extract::Query), rejection(MyQueryRejection))]
#[serde(transparent)]
struct MyQuery(HashMap<String, String>);

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
