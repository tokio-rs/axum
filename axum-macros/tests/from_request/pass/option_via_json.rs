use axum::{
    extract::rejection::{JsonRejection, PathRejection},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use axum_macros::FromRequest;
use serde::Deserialize;

fn main() {
    let _: Router = Router::new().route("/", get(handler));
}

async fn handler(_: Args) {}

#[derive(Deserialize)]
struct Payload {
    value: String,
}

// Test case from issue #3623:
// Option<T> with via(Json) should use OptionalFromRequest,
// not .ok() which silently swallows rejections.
#[derive(FromRequest)]
#[from_request(rejection(MyError))]
struct Args {
    #[from_request(via(axum::extract::Path))]
    something: String,
    #[from_request(via(Json))]
    request: Option<Payload>,
}

struct MyError(Response);

impl From<PathRejection> for MyError {
    fn from(rejection: PathRejection) -> Self {
        Self(rejection.into_response())
    }
}

impl From<JsonRejection> for MyError {
    fn from(rejection: JsonRejection) -> Self {
        Self(rejection.into_response())
    }
}

impl IntoResponse for MyError {
    fn into_response(self) -> Response {
        self.0
    }
}
