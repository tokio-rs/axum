use axum::extract::MatchedPath;
use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, RequestParts},
    http::StatusCode,
    response::IntoResponse,
    BoxError,
};
use chrono::Utc;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

pub async fn handler(Json(value): Json<Value>) -> impl IntoResponse {
    Json(dbg!(value));
}

// We define our own `Json` extractor that customizes the error from `axum::Json`
pub struct Json<T>(T);

#[async_trait]
impl<S, B, T> FromRequest<S, B> for Json<T>
where
    S: Send + Sync,
    // these trait bounds are copied from `impl FromRequest for axum::Json`
    T: DeserializeOwned,
    B: axum::body::HttpBody + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = (StatusCode, axum::Json<Value>);

    async fn from_request(req: &mut RequestParts<S, B>) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req).await {
            Ok(value) => Ok(Self(value.0)),
            Err(rejection) => {
                // convert the error from `axum::Json` into whatever we want
                let path = req
                    .extensions()
                    .get::<MatchedPath>()
                    .map(|x| x.as_str().to_owned());

                let payload = json!({
                    "message": rejection.to_string(),
                    "timestamp": Utc::now(),
                    "origin": "custom_extractor",
                    "path": path,
                });

                let code = match rejection {
                    JsonRejection::JsonDataError(_) | JsonRejection::MissingJsonContentType(_) => {
                        StatusCode::BAD_REQUEST
                    }
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                Err((code, axum::Json(payload)))
            }
        }
    }
}
