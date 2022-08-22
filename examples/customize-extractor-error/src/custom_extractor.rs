//! Manual implementation of `FromRequest` that wraps another extractor
//!
//! + Powerful API: Implementing `FromRequest` grants access to `RequestParts`
//!   and `async/await`. This means that you can create more powerful rejections
//! - Boilerplate: Requires creating a new extractor for every custom rejection
//! - Complexity: Manually implementing `FromRequest` results on more complex code
use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, FromRequestParts, MatchedPath},
    http::Request,
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::{json, Value};

pub async fn handler(Json(value): Json<Value>) -> impl IntoResponse {
    Json(dbg!(value));
}

// We define our own `Json` extractor that customizes the error from `axum::Json`
pub struct Json<T>(pub T);

#[async_trait]
impl<S, B, T> FromRequest<S, B> for Json<T>
where
    axum::Json<T>: FromRequest<S, B, Rejection = JsonRejection>,
    S: Send + Sync,
    B: Send + 'static,
{
    type Rejection = (StatusCode, axum::Json<Value>);

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let (mut parts, body) = req.into_parts();

        // We can use other extractors to provide better rejection
        // messages. For example, here we are using
        // `axum::extract::MatchedPath` to provide a better error
        // message
        //
        // Have to run that first since `Json::from_request` consumes
        // the request
        let path = MatchedPath::from_request_parts(&mut parts, state)
            .await
            .map(|path| path.as_str().to_owned())
            .ok();

        let req = Request::from_parts(parts, body);

        match axum::Json::<T>::from_request(req, state).await {
            Ok(value) => Ok(Self(value.0)),
            // convert the error from `axum::Json` into whatever we want
            Err(rejection) => {
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
