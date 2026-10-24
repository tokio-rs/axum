use axum::{extract::rejection::JsonRejection, Json};
use axum_extra::extract::WithRejection;
use axum_macros::debug_handler;
use serde::Deserialize;

// A rejection type that does NOT implement IntoResponse.
struct BadRejection;

impl From<JsonRejection> for BadRejection {
    fn from(_: JsonRejection) -> Self {
        Self
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
