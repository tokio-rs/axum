use axum::{
    extract::rejection::JsonRejection,
    response::IntoResponse,
    Json,
};
use axum_extra::extract::WithRejection;
use axum_macros::debug_handler;
use serde::Deserialize;

// A custom rejection type that wraps the inner rejection.
struct MyError(JsonRejection);

impl From<JsonRejection> for MyError {
    fn from(rejection: JsonRejection) -> Self {
        Self(rejection)
    }
}

impl IntoResponse for MyError {
    fn into_response(self) -> axum::response::Response {
        self.0.into_response()
    }
}

#[derive(Deserialize)]
struct Payload {
    value: String,
}

// WithRejection as the only extractor (body-consuming inner type)
#[debug_handler]
async fn handler_json(
    WithRejection(payload, _): WithRejection<Json<Payload>, MyError>,
) {
    let _ = payload;
}

fn main() {}
