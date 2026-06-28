use super::{rejection::*, FromRequest, FromRequestParts, Request};
use crate::{body::Body, RequestExt};
use bytes::{BufMut, Bytes, BytesMut};
use http::{request::Parts, Extensions, HeaderMap, Method, Uri, Version};
use http_body_util::BodyExt;
use std::convert::Infallible;

impl<S> FromRequest<S> for Request
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req)
    }
}

impl<S> FromRequestParts<S> for Method
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.method.clone())
    }
}

impl<S> FromRequestParts<S> for Uri
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.uri.clone())
    }
}

impl<S> FromRequestParts<S> for Version
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.version)
    }
}

/// Clone the headers from the request.
///
/// Prefer using [`TypedHeader`] to extract only the headers you need.
///
/// [`TypedHeader`]: https://docs.rs/axum-extra/0.10/axum_extra/struct.TypedHeader.html
impl<S> FromRequestParts<S> for HeaderMap
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.headers.clone())
    }
}

#[diagnostic::do_not_recommend] // pretty niche impl
impl<S> FromRequest<S> for BytesMut
where
    S: Send + Sync,
{
    type Rejection = BytesRejection;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        let mut body = req.into_limited_body();
        #[allow(clippy::use_self)]
        let mut bytes = BytesMut::new();
        body_to_bytes_mut(&mut body, &mut bytes).await?;
        Ok(bytes)
    }
}

async fn body_to_bytes_mut(body: &mut Body, bytes: &mut BytesMut) -> Result<(), BytesRejection> {
    while let Some(frame) = body
        .frame()
        .await
        .transpose()
        .map_err(FailedToBufferBody::from_err)?
    {
        let Ok(data) = frame.into_data() else {
            continue;
        };
        bytes.put(data);
    }

    Ok(())
}

impl<S> FromRequest<S> for Bytes
where
    S: Send + Sync,
{
    type Rejection = BytesRejection;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        let bytes = req
            .into_limited_body()
            .collect()
            .await
            .map_err(FailedToBufferBody::from_err)?
            .to_bytes();

        Ok(bytes)
    }
}

impl<S> FromRequest<S> for String
where
    S: Send + Sync,
{
    type Rejection = StringRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|err| match err {
                BytesRejection::FailedToBufferBody(inner) => {
                    StringRejection::FailedToBufferBody(inner)
                }
            })?;

        #[allow(clippy::use_self)]
        let string = String::from_utf8(bytes.into()).map_err(InvalidUtf8::from_err)?;

        Ok(string)
    }
}

#[diagnostic::do_not_recommend] // pretty niche impl
impl<S> FromRequestParts<S> for Parts
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.clone())
    }
}

#[diagnostic::do_not_recommend] // pretty niche impl
impl<S> FromRequestParts<S> for Extensions
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.extensions.clone())
    }
}

impl<S> FromRequest<S> for Body
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        Ok(req.into_body())
    }
}

#[cfg(test)]
mod tests {
    use super::{Bytes, BytesMut, FromRequest, Request};
    use crate::body::Body;
    use axum::{extract::Extension, routing::get, test_helpers::*, Router};
    use bytes::Bytes as RawBytes;
    use http::{HeaderMap, Method, StatusCode};
    use http_body::Frame;
    use std::{
        collections::VecDeque,
        convert::Infallible,
        pin::Pin,
        task::{Context, Poll},
    };

    #[crate::test]
    async fn extract_request_parts() {
        #[derive(Clone)]
        struct Ext;

        async fn handler(parts: http::request::Parts) {
            assert_eq!(parts.method, Method::GET);
            assert_eq!(parts.uri, "/");
            assert_eq!(parts.version, http::Version::HTTP_11);
            assert_eq!(parts.headers["x-foo"], "123");
            parts.extensions.get::<Ext>().unwrap();
        }

        let client = TestClient::new(Router::new().route("/", get(handler)).layer(Extension(Ext)));

        let res = client.get("/").header("x-foo", "123").await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[crate::test]
    async fn bytes_mut_extractor_skips_non_data_frames() {
        struct InterleavedFramesBody {
            frames: VecDeque<Result<Frame<RawBytes>, Infallible>>,
        }

        impl http_body::Body for InterleavedFramesBody {
            type Data = RawBytes;
            type Error = Infallible;

            fn poll_frame(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
                Poll::Ready(self.get_mut().frames.pop_front())
            }
        }

        fn request() -> Request {
            let body = InterleavedFramesBody {
                frames: VecDeque::from(vec![
                    Ok(Frame::data(RawBytes::from_static(b"hello "))),
                    Ok(Frame::trailers(HeaderMap::new())),
                    Ok(Frame::data(RawBytes::from_static(b"world"))),
                ]),
            };

            Request::builder()
                .body(Body::new(body))
                .expect("request should build")
        }

        let bytes_mut = BytesMut::from_request(request(), &()).await.unwrap();
        let bytes = Bytes::from_request(request(), &()).await.unwrap();

        assert_eq!(&bytes_mut[..], b"hello world");
        assert_eq!(&bytes[..], b"hello world");
    }
}
