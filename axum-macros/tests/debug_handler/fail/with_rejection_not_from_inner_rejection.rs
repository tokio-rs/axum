use axum::{response::IntoResponse, Json};
use axum_extra::extract::WithRejection;
use axum_macros::debug_handler;
use serde::Deserialize;

// A rejection type that implements IntoResponse but does NOT implement
// `From<JsonRejection>`.
struct BadRejection;

impl IntoResponse for BadRejection {
    fn into_response(self) -> axum::response::Response {
        ().into_response()
    }
}

#[derive(Deserialize)]
struct Payload {
    value: String,
}

#[debug_handler]
async fn handler(
    WithRejection(payload, _): WithRejection<Json<Payload>, BadRejection>,
) {
    let _ = payload;
}

fn main() {}
