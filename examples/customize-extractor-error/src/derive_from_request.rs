//! Uses `axum_macros::FromRequest` to wrap another extractor and customize the
//! rejection
//!
//! + Easy learning curve: Deriving `FromRequest` generates a `FromRequest`
//!   implementation for your type using another extractor. You only need
//!   to provide a `From` impl between the original rejection type and the
//!   target rejection. Crates like [`thiserror`] can provide such conversion
//!   using derive macros.
//! - Boilerplate: Requires deriving `FromRequest` for every custom rejection
//! - There are some known limitations: [FromRequest#known-limitations]
//!
//! [`thiserror`]: https://crates.io/crates/thiserror
//! [FromRequest#known-limitations]: https://docs.rs/axum-macros/*/axum_macros/derive.FromRequest.html#known-limitations
use axum::{extract::rejection::JsonRejection, http::StatusCode, response::IntoResponse};
use axum_macros::FromRequest;
use serde_json::{json, Value};

pub async fn handler(Json(value): Json<Value>) -> impl IntoResponse {
    Json(dbg!(value));
}

// create an extractor that internally uses `axum::Json` but has a custom rejection
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Json<T>(T);

// We create our own rejection type
#[derive(Debug)]
pub struct ApiError {
    code: StatusCode,
    message: String,
}

// We implement `From<JsonRejection> for ApiError`
impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        let code = match rejection {
            JsonRejection::JsonDataError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            JsonRejection::JsonSyntaxError(_) => StatusCode::BAD_REQUEST,
            JsonRejection::MissingJsonContentType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            code,
            message: rejection.to_string(),
        }
    }
}

// We implement `IntoResponse` so `ApiError` can be used as a response
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let payload = json!({
            "message": self.message,
            "origin": "derive_from_request"
        });

        (self.code, axum::Json(payload)).into_response()
    }
}
