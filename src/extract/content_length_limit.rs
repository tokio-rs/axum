use crate::response::IntoResponse;

use super::{rejection::*, FromRequest, RequestParts};
use async_trait::async_trait;
use std::ops::Deref;

/// Extractor that will reject requests with a body larger than some size.
///
/// # Example
///
/// ```rust,no_run
/// use axum::prelude::*;
///
/// async fn handler(body: extract::ContentLengthLimit<String, 1024>) {
///     // ...
/// }
///
/// let app = route("/", post(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// This requires the request to have a `Content-Length` header.
#[derive(Debug, Clone)]
pub struct ContentLengthLimit<T, const N: u64>(pub T);

#[async_trait]
impl<T, B, const N: u64> FromRequest<B> for ContentLengthLimit<T, N>
where
    T: FromRequest<B>,
    T::Rejection: IntoResponse,
    B: Send,
{
    type Rejection = ContentLengthLimitRejection<T::Rejection>;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let content_length = req
            .headers()
            .ok_or(ContentLengthLimitRejection::HeadersAlreadyExtracted(
                HeadersAlreadyExtracted,
            ))?
            .get(http::header::CONTENT_LENGTH);

        let content_length =
            content_length.and_then(|value| value.to_str().ok()?.parse::<u64>().ok());

        if let Some(length) = content_length {
            if length > N {
                return Err(ContentLengthLimitRejection::PayloadTooLarge(
                    PayloadTooLarge,
                ));
            }
        } else {
            return Err(ContentLengthLimitRejection::LengthRequired(LengthRequired));
        };

        let value = T::from_request(req)
            .await
            .map_err(ContentLengthLimitRejection::Inner)?;

        Ok(Self(value))
    }
}

impl<T, const N: u64> Deref for ContentLengthLimit<T, N> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
