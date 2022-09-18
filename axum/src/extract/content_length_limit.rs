use super::{rejection::*, FromRequest};
use async_trait::async_trait;
use axum_core::{extract::FromRequestParts, response::IntoResponse};
use http::{request::Parts, Method, Request};
use http_body::Limited;
use std::ops::Deref;

/// Extractor that will reject requests with a body larger than some size.
///
/// `GET`, `HEAD`, and `OPTIONS` requests are rejected if they have a `Content-Length` header,
/// otherwise they're accepted without the body being checked.
///
/// Note: `ContentLengthLimit` can wrap types that extract the body (for example, [`Form`] or [`Json`])
/// if that is the case, the inner type will consume the request's body, which means the
/// `ContentLengthLimit` must come *last* if the handler uses several extractors. See
/// ["the order of extractors"][order-of-extractors]
///
/// [order-of-extractors]: crate::extract#the-order-of-extractors
/// [`Form`]: crate::form::Form
/// [`Json`]: crate::json::Json
///
/// # Example
///
/// ```rust,no_run
/// use axum::{
///     extract::ContentLengthLimit,
///     routing::post,
///     Router,
/// };
///
/// async fn handler(body: ContentLengthLimit<String, 1024>) {
///     // ...
/// }
///
/// let app = Router::new().route("/", post(handler));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[derive(Debug, Clone)]
pub struct ContentLengthLimit<T, const N: u64>(pub T);

#[async_trait]
impl<T, S, B, R, const N: u64> FromRequest<S, B> for ContentLengthLimit<T, N>
where
    T: FromRequest<S, B, Rejection = R> + FromRequest<S, Limited<B>, Rejection = R>,
    R: IntoResponse + Send,
    B: Send + 'static,
    S: Send + Sync,
{
    type Rejection = ContentLengthLimitRejection<R>;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let (parts, body) = req.into_parts();

        let value = if let Some(err) = validate::<N>(&parts).err() {
            match err {
                RequestValidationError::LengthRequiredStream => {
                    // `Limited` supports limiting streams, so use that instead since this is a
                    // streaming request
                    let body = Limited::new(body, N as usize);
                    let req = Request::from_parts(parts, body);
                    T::from_request(req, state)
                        .await
                        .map_err(ContentLengthLimitRejection::Inner)?
                }
                other => return Err(other.into()),
            }
        } else {
            let req = Request::from_parts(parts, body);
            T::from_request(req, state)
                .await
                .map_err(ContentLengthLimitRejection::Inner)?
        };

        Ok(Self(value))
    }
}

#[async_trait]
impl<T, S, const N: u64> FromRequestParts<S> for ContentLengthLimit<T, N>
where
    T: FromRequestParts<S>,
    T::Rejection: IntoResponse,
    S: Send + Sync,
{
    type Rejection = ContentLengthLimitRejection<T::Rejection>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        validate::<N>(parts)?;

        let value = T::from_request_parts(parts, state)
            .await
            .map_err(ContentLengthLimitRejection::Inner)?;

        Ok(Self(value))
    }
}

fn validate<const N: u64>(parts: &Parts) -> Result<(), RequestValidationError> {
    let content_length = parts
        .headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok()?.parse::<u64>().ok());

    match (content_length, &parts.method) {
        (content_length, &(Method::GET | Method::HEAD | Method::OPTIONS)) => {
            if content_length.is_some() {
                return Err(RequestValidationError::ContentLengthNotAllowed);
            } else if parts
                .headers
                .get(http::header::TRANSFER_ENCODING)
                .map_or(false, |value| value.as_bytes() == b"chunked")
            {
                return Err(RequestValidationError::LengthRequiredChunkedHeadOrGet);
            }
        }
        (Some(content_length), _) if content_length > N => {
            return Err(RequestValidationError::PayloadTooLarge);
        }
        (None, _) => {
            return Err(RequestValidationError::LengthRequiredStream);
        }
        _ => {}
    }

    Ok(())
}

impl<T, const N: u64> Deref for ContentLengthLimit<T, N> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Similar to `ContentLengthLimitRejection` but more fine grained in that we can tell the
/// difference between `LengthRequiredStream` and `LengthRequiredChunkedHeadOrGet`
enum RequestValidationError {
    PayloadTooLarge,
    LengthRequiredStream,
    LengthRequiredChunkedHeadOrGet,
    ContentLengthNotAllowed,
}

impl<T> From<RequestValidationError> for ContentLengthLimitRejection<T> {
    fn from(inner: RequestValidationError) -> Self {
        match inner {
            RequestValidationError::PayloadTooLarge => Self::PayloadTooLarge(PayloadTooLarge),
            RequestValidationError::LengthRequiredStream
            | RequestValidationError::LengthRequiredChunkedHeadOrGet => {
                Self::LengthRequired(LengthRequired)
            }
            RequestValidationError::ContentLengthNotAllowed => {
                Self::ContentLengthNotAllowed(ContentLengthNotAllowed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        body::Bytes,
        routing::{get, post},
        test_helpers::*,
        Router,
    };
    use http::StatusCode;
    use serde::Deserialize;

    #[tokio::test]
    async fn body_with_length_limit() {
        use std::iter::repeat;

        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct Input {
            foo: String,
        }

        const LIMIT: u64 = 8;

        let app = Router::new().route(
            "/",
            post(|_body: ContentLengthLimit<Bytes, LIMIT>| async {}),
        );

        let client = TestClient::new(app);
        let res = client
            .post("/")
            .body(repeat(0_u8).take((LIMIT - 1) as usize).collect::<Vec<_>>())
            .send()
            .await;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client
            .post("/")
            .body(repeat(0_u8).take(LIMIT as usize).collect::<Vec<_>>())
            .send()
            .await;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client
            .post("/")
            .body(repeat(0_u8).take((LIMIT + 1) as usize).collect::<Vec<_>>())
            .send()
            .await;
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let chunk = repeat(0_u8).take(LIMIT as usize).collect::<Bytes>();
        let res = client
            .post("/")
            .body(reqwest::Body::wrap_stream(futures_util::stream::iter(
                vec![Ok::<_, std::io::Error>(chunk)],
            )))
            .send()
            .await;
        assert_eq!(res.status(), StatusCode::OK);

        let chunk = repeat(0_u8).take((LIMIT + 1) as usize).collect::<Bytes>();
        let res = client
            .post("/")
            .body(reqwest::Body::wrap_stream(futures_util::stream::iter(
                vec![Ok::<_, std::io::Error>(chunk)],
            )))
            .send()
            .await;
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn get_request_without_content_length_is_accepted() {
        let app = Router::new().route("/", get(|_body: ContentLengthLimit<Bytes, 1337>| async {}));

        let client = TestClient::new(app);

        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_request_with_content_length_is_rejected() {
        let app = Router::new().route("/", get(|_body: ContentLengthLimit<Bytes, 1337>| async {}));

        let client = TestClient::new(app);

        let res = client
            .get("/")
            .header("content-length", 3)
            .body("foo")
            .send()
            .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn get_request_with_chunked_encoding_is_rejected() {
        let app = Router::new().route("/", get(|_body: ContentLengthLimit<Bytes, 1337>| async {}));

        let client = TestClient::new(app);

        let res = client
            .get("/")
            .header("transfer-encoding", "chunked")
            .body("3\r\nfoo\r\n0\r\n\r\n")
            .send()
            .await;

        assert_eq!(res.status(), StatusCode::LENGTH_REQUIRED);
    }
}
