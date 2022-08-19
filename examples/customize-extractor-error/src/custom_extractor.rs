//! Manual implementation of `FromRequest` that wraps another extractor
//!
//! + Powerful API: Implementing `FromRequest` grants access to `RequestParts`
//!   and `async/await`. This means that you can create more powerful rejections
//! - Boilerplate: Requires creating a new extractor for every custom rejection
//! - Complexity: Manually implementing `FromRequest` results on more complex code
use axum::extract::MatchedPath;
use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, RequestParts},
    http::StatusCode,
    response::IntoResponse,
    BoxError,
};
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
    // `T: Send` is required to send this future across an await
    T: DeserializeOwned + Send,
    B: axum::body::HttpBody + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = (StatusCode, axum::Json<Value>);

    async fn from_request(req: &mut RequestParts<S, B>) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req).await {
            Ok(value) => Ok(Self(value.0)),
            // convert the error from `axum::Json` into whatever we want
            Err(rejection) => {
                let path = req
                    .extract::<MatchedPath>()
                    .await
                    .map(|x| x.as_str().to_owned())
                    .ok();

                // We can use other extractors to provide better rejection
                // messages. For example, here we are using
                // `axum::extract::MatchedPath` to provide a better error
                // message
                let payload = json!({
                    "message": rejection.to_string(),
                    "origin": "custom_extractor",
                    "path": path,
                });

                let code = match rejection {
                    JsonRejection::JsonDataError(_) => StatusCode::UNPROCESSABLE_ENTITY,
                    JsonRejection::JsonSyntaxError(_) => StatusCode::BAD_REQUEST,
                    JsonRejection::MissingJsonContentType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                Err((code, axum::Json(payload)))
            }
        }
    }
}
