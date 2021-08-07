use crate::{
    extract::{has_content_type, rejection::*, take_body, FromRequest, RequestParts},
    prelude::response::IntoResponse,
    Body,
};
use async_trait::async_trait;
use http::{
    header::{self, HeaderValue},
    StatusCode,
};
use hyper::Response;
use serde::{de::DeserializeOwned, Serialize};
use std::ops::Deref;

/// JSON Extractor/Response
///
/// When used as an Extractor, it can deserializes request bodies into some type that
/// implements [`serde::Serialize`]. If the request bodies cannot be parsed, or it does not contain
/// the `Content-Type: application/json`t header, will reject the request and return a
/// `400 Bad Request` response.
///
/// # Extractor example
///
/// ```rust,no_run
/// use axum::prelude::*;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct CreateUser {
///     email: String,
///     password: String,
/// }
///
/// async fn create_user(payload: extract::Json<CreateUser>) {
///     let payload: CreateUser = payload.0;
///
///     // ...
/// }
///
/// let app = route("/users", post(create_user));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// When used as a `Response`, it can serialize any type that implements [`serde::Serialize`] to `JSON`,
/// and will automatically set `Content-Type: application/json` header.
///
/// # Response example
///
/// ```
/// use serde_json::json;
/// use axum::{body::Body, response::{Json, IntoResponse}};
/// use http::{Response, header::CONTENT_TYPE};
///
/// let json = json!({
///     "data": 42,
/// });
///
/// let response: Response<Body> = Json(json).into_response();
///
/// assert_eq!(
///     response.headers().get(CONTENT_TYPE).unwrap(),
///     "application/json",
/// );
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for Json<T>
where
    T: DeserializeOwned,
    B: http_body::Body + Send,
    B::Data: Send,
    B::Error: Into<tower::BoxError>,
{
    type Rejection = JsonRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        use bytes::Buf;

        if has_content_type(req, "application/json")? {
            let body = take_body(req)?;

            let buf = hyper::body::aggregate(body)
                .await
                .map_err(InvalidJsonBody::from_err)?;

            let value = serde_json::from_reader(buf.reader()).map_err(InvalidJsonBody::from_err)?;

            Ok(Json(value))
        } else {
            Err(MissingJsonContentType.into())
        }
    }
}

impl<T> Deref for Json<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> From<T> for Json<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> IntoResponse for Json<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response<Body> {
        let bytes = match serde_json::to_vec(&self.0) {
            Ok(res) => res,
            Err(err) => {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from(err.to_string()))
                    .unwrap();
            }
        };

        let mut res = Response::new(Body::from(bytes));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        res
    }
}
