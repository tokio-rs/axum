use axum::{extract::rejection::JsonRejection, response::IntoResponse, Json};
use axum_extra::extract::WithRejection;
use chrono::Utc;
use serde_json::{json, Value};
use thiserror::Error;

pub async fn handler(
    WithRejection(Json(value), _): WithRejection<Json<Value>, ApiError>,
) -> impl IntoResponse {
    dbg!(value);
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    JsonExtractorRejection(#[from] JsonRejection),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let payload = json!({
            "message": self.to_string(),
            "timestamp": Utc::now(),
            "origin": "with_rejection"
        });
        Json(payload).into_response()
    }
}
