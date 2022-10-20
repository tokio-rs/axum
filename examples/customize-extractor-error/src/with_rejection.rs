//! Uses `axum_extra::extract::WithRejection` to transform one rejection into
//! another
//!
//! + Easy learning curve: `WithRejection` acts as a wrapper for another
//!   already existing extractor. You only need to provide a `From` impl
//!   between the original rejection type and the target rejection. Crates like
//!   `thiserror` can provide such conversion using derive macros. See
//!   [`thiserror`]
//! - Verbose types: types become much larger, which makes them difficult to
//!   read. Current limitations on type aliasing makes impossible to destructure
//!   a type alias. See [#1116]
//!   
//! [`thiserror`]: https://crates.io/crates/thiserror
//! [#1116]: https://github.com/tokio-rs/axum/issues/1116#issuecomment-1186197684

use axum::{extract::rejection::JsonRejection, http::StatusCode, response::IntoResponse, Json};
use axum_extra::extract::WithRejection;
use serde_json::{json, Value};
use thiserror::Error;

pub async fn handler(
    // `WithRejection` will extract `Json<Value>` from the request. If it fails,
    // `JsonRejection` will be transform into `ApiError` and returned as response
    // to the client.
    //
    // The second constructor argument is not meaningful and can be safely ignored
    WithRejection(Json(value), _): WithRejection<Json<Value>, ApiError>,
) -> impl IntoResponse {
    Json(dbg!(value))
}

// We derive `thiserror::Error`
#[derive(Debug, Error)]
pub enum ApiError {
    // The `#[from]` attribute generates `From<JsonRejection> for ApiError`
    // implementation. See `thiserror` docs for more information
    #[error(transparent)]
    JsonExtractorRejection(#[from] JsonRejection),
}
// We implement `IntoResponse` so ApiError can be used as a response
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let payload = json!({
            "message": self.to_string(),
            "origin": "with_rejection"
        });
        let code = match self {
            ApiError::JsonExtractorRejection(x) => match x {
                JsonRejection::JsonDataError(_) => StatusCode::UNPROCESSABLE_ENTITY,
                JsonRejection::JsonSyntaxError(_) => StatusCode::BAD_REQUEST,
                JsonRejection::MissingJsonContentType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
        };
        (code, Json(payload)).into_response()
    }
}
