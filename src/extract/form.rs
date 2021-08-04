use super::{has_content_type, rejection::*, take_body, FromRequest, RequestParts};
use async_trait::async_trait;
use bytes::Buf;
use http::Method;
use serde::de::DeserializeOwned;
use std::ops::Deref;

/// Extractor that deserializes `application/x-www-form-urlencoded` requests
/// into some type.
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
/// struct SignUp {
///     username: String,
///     password: String,
/// }
///
/// async fn accept_form(form: extract::Form<SignUp>) {
///     let sign_up: SignUp = form.0;
///
///     // ...
/// }
///
/// let app = route("/sign_up", post(accept_form));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Note that `Content-Type: multipart/form-data` requests are not supported.
#[derive(Debug, Clone, Copy, Default)]
pub struct Form<T>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for Form<T>
where
    T: DeserializeOwned,
    B: http_body::Body + Send,
    B::Data: Send,
    B::Error: Into<tower::BoxError>,
{
    type Rejection = FormRejection;

    #[allow(warnings)]
    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if !has_content_type(&req, "application/x-www-form-urlencoded")? {
            Err(InvalidFormContentType)?;
        }

        if req.method().ok_or(MethodAlreadyExtracted)? == Method::GET {
            let query = req
                .uri()
                .ok_or(UriAlreadyExtracted)?
                .query()
                .ok_or(QueryStringMissing)?;
            let value = serde_urlencoded::from_str(query)
                .map_err(FailedToDeserializeQueryString::new::<T, _>)?;
            Ok(Form(value))
        } else {
            let body = take_body(req)?;
            let chunks = hyper::body::aggregate(body)
                .await
                .map_err(FailedToBufferBody::from_err)?;
            let value = serde_urlencoded::from_reader(chunks.reader())
                .map_err(FailedToDeserializeQueryString::new::<T, _>)?;

            Ok(Form(value))
        }
    }
}

impl<T> Deref for Form<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
