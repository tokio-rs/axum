use super::{rejection::*, FromRequest, RequestParts};
use crate::body::BoxBody;
use async_trait::async_trait;
use bytes::Bytes;
use http::{Extensions, HeaderMap, Method, Request, Uri, Version};
use std::convert::Infallible;

#[async_trait]
impl FromRequest for Request<BoxBody> {
    type Rejection = BodyAlreadyExtracted;

    async fn from_request(req: &mut RequestParts) -> Result<Self, Self::Rejection> {
        let req = std::mem::replace(
            req,
            RequestParts {
                method: req.method.clone(),
                version: req.version,
                uri: req.uri.clone(),
                headers: HeaderMap::new(),
                extensions: Extensions::default(),
                body: None,
            },
        );

        req.try_into_request()
    }
}

#[async_trait]
impl FromRequest for Method {
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts) -> Result<Self, Self::Rejection> {
        Ok(req.method().clone())
    }
}

#[async_trait]
impl FromRequest for Uri {
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts) -> Result<Self, Self::Rejection> {
        Ok(req.uri().clone())
    }
}

#[async_trait]
impl FromRequest for Version {
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts) -> Result<Self, Self::Rejection> {
        Ok(req.version())
    }
}

/// Clone the headers from the request.
///
/// Prefer using [`TypedHeader`] to extract only the headers you need.
///
/// [`TypedHeader`]: https://docs.rs/axum/latest/axum/extract/struct.TypedHeader.html
#[async_trait]
impl FromRequest for HeaderMap {
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts) -> Result<Self, Self::Rejection> {
        Ok(req.headers().clone())
    }
}

#[async_trait]
impl FromRequest for BoxBody {
    type Rejection = BodyAlreadyExtracted;

    async fn from_request(req: &mut RequestParts) -> Result<Self, Self::Rejection> {
        Ok(take_body(req)?)
    }
}

#[async_trait]
impl FromRequest for Bytes {
    type Rejection = BytesRejection;

    async fn from_request(req: &mut RequestParts) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;

        let bytes = crate::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)?;

        Ok(bytes)
    }
}

#[async_trait]
impl FromRequest for String {
    type Rejection = StringRejection;

    async fn from_request(req: &mut RequestParts) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;

        let bytes = crate::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)?
            .to_vec();

        let string = String::from_utf8(bytes).map_err(InvalidUtf8::from_err)?;

        Ok(string)
    }
}

#[async_trait]
impl FromRequest for http::request::Parts {
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts) -> Result<Self, Self::Rejection> {
        let method = unwrap_infallible(Method::from_request(req).await);
        let uri = unwrap_infallible(Uri::from_request(req).await);
        let version = unwrap_infallible(Version::from_request(req).await);
        let headers = unwrap_infallible(HeaderMap::from_request(req).await);
        let extensions = std::mem::take(req.extensions_mut());

        let mut temp_request = Request::new(());
        *temp_request.method_mut() = method;
        *temp_request.uri_mut() = uri;
        *temp_request.version_mut() = version;
        *temp_request.headers_mut() = headers;
        *temp_request.extensions_mut() = extensions;

        let (parts, _) = temp_request.into_parts();

        Ok(parts)
    }
}

fn unwrap_infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => match err {},
    }
}

pub(crate) fn take_body(req: &mut RequestParts) -> Result<BoxBody, BodyAlreadyExtracted> {
    req.take_body().ok_or(BodyAlreadyExtracted)
}
