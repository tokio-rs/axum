use super::{has_content_type, rejection::*, take_body, FromRequest, RequestParts};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::ops::Deref;

/// Extractor that deserializes request bodies into some type.
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Example
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
/// If the query string cannot be parsed it will reject the request with a `400
/// Bad Request` response.
///
/// The request is required to have a `Content-Type: application/json` header.
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
