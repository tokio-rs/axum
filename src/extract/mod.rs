use crate::{body::Body, response::IntoResponse};
use async_trait::async_trait;
use bytes::Bytes;
use http::{header, Request, Response};
use rejection::{
    BodyAlreadyTaken, FailedToBufferBody, InvalidJsonBody, InvalidUtf8, LengthRequired,
    MissingExtension, MissingJsonContentType, MissingRouteParams, PayloadTooLarge,
    QueryStringMissing,
};
use serde::de::DeserializeOwned;
use std::{collections::HashMap, convert::Infallible, str::FromStr};

pub mod rejection;

#[async_trait]
pub trait FromRequest<B>: Sized {
    type Rejection: IntoResponse<B>;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection>;
}

#[async_trait]
impl<T, B> FromRequest<B> for Option<T>
where
    T: FromRequest<B>,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut Request<Body>) -> Result<Option<T>, Self::Rejection> {
        Ok(T::from_request(req).await.ok())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

#[async_trait]
impl<T> FromRequest<Body> for Query<T>
where
    T: DeserializeOwned,
{
    type Rejection = QueryStringMissing;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        let query = req.uri().query().ok_or(QueryStringMissing(()))?;
        let value = serde_urlencoded::from_str(query).map_err(|_| QueryStringMissing(()))?;
        Ok(Query(value))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

#[async_trait]
impl<T> FromRequest<Body> for Json<T>
where
    T: DeserializeOwned,
{
    type Rejection = Response<Body>;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        if has_content_type(req, "application/json") {
            let body = take_body(req).map_err(IntoResponse::into_response)?;

            let bytes = hyper::body::to_bytes(body)
                .await
                .map_err(InvalidJsonBody::from_err)
                .map_err(IntoResponse::into_response)?;

            let value = serde_json::from_slice(&bytes)
                .map_err(InvalidJsonBody::from_err)
                .map_err(IntoResponse::into_response)?;

            Ok(Json(value))
        } else {
            Err(MissingJsonContentType(()).into_response())
        }
    }
}

fn has_content_type<B>(req: &Request<B>, expected_content_type: &str) -> bool {
    let content_type = if let Some(content_type) = req.headers().get(header::CONTENT_TYPE) {
        content_type
    } else {
        return false;
    };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return false;
    };

    content_type.starts_with(expected_content_type)
}

#[derive(Debug, Clone, Copy)]
pub struct Extension<T>(pub T);

#[async_trait]
impl<T> FromRequest<Body> for Extension<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Rejection = MissingExtension;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        let value = req
            .extensions()
            .get::<T>()
            .ok_or(MissingExtension(()))
            .map(|x| x.clone())?;

        Ok(Extension(value))
    }
}

#[async_trait]
impl FromRequest<Body> for Bytes {
    type Rejection = Response<Body>;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        let body = take_body(req).map_err(IntoResponse::into_response)?;

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)
            .map_err(IntoResponse::into_response)?;

        Ok(bytes)
    }
}

#[async_trait]
impl FromRequest<Body> for String {
    type Rejection = Response<Body>;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        let body = take_body(req).map_err(IntoResponse::into_response)?;

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)
            .map_err(IntoResponse::into_response)?
            .to_vec();

        let string = String::from_utf8(bytes)
            .map_err(InvalidUtf8::from_err)
            .map_err(IntoResponse::into_response)?;

        Ok(string)
    }
}

#[async_trait]
impl FromRequest<Body> for Body {
    type Rejection = BodyAlreadyTaken;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        take_body(req)
    }
}

#[derive(Debug, Clone)]
pub struct BytesMaxLength<const N: u64>(pub Bytes);

#[async_trait]
impl<const N: u64> FromRequest<Body> for BytesMaxLength<N> {
    type Rejection = Response<Body>;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        let content_length = req.headers().get(http::header::CONTENT_LENGTH).cloned();
        let body = take_body(req).map_err(|reject| reject.into_response())?;

        let content_length =
            content_length.and_then(|value| value.to_str().ok()?.parse::<u64>().ok());

        if let Some(length) = content_length {
            if length > N {
                return Err(PayloadTooLarge(()).into_response());
            }
        } else {
            return Err(LengthRequired(()).into_response());
        };

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(|e| FailedToBufferBody::from_err(e).into_response())?;

        Ok(BytesMaxLength(bytes))
    }
}

#[derive(Debug)]
pub struct UrlParamsMap(HashMap<String, String>);

impl UrlParamsMap {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|s| &**s)
    }

    pub fn get_typed<T>(&self, key: &str) -> Option<T>
    where
        T: FromStr,
    {
        self.get(key)?.parse().ok()
    }
}

#[async_trait]
impl FromRequest<Body> for UrlParamsMap {
    type Rejection = MissingRouteParams;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        if let Some(params) = req
            .extensions_mut()
            .get_mut::<Option<crate::routing::UrlParams>>()
        {
            let params = params.take().expect("params already taken").0;
            Ok(Self(params.into_iter().collect()))
        } else {
            Err(MissingRouteParams(()))
        }
    }
}

#[derive(Debug)]
pub struct InvalidUrlParam {
    type_name: &'static str,
}

impl InvalidUrlParam {
    fn new<T>() -> Self {
        InvalidUrlParam {
            type_name: std::any::type_name::<T>(),
        }
    }
}

impl IntoResponse<Body> for InvalidUrlParam {
    fn into_response(self) -> http::Response<Body> {
        let mut res = http::Response::new(Body::from(format!(
            "Invalid URL param. Expected something of type `{}`",
            self.type_name
        )));
        *res.status_mut() = http::StatusCode::BAD_REQUEST;
        res
    }
}

pub struct UrlParams<T>(pub T);

macro_rules! impl_parse_url {
    () => {};

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        impl<$head, $($tail,)*> FromRequest<Body> for UrlParams<($head, $($tail,)*)>
        where
            $head: FromStr + Send,
            $( $tail: FromStr + Send, )*
        {
            type Rejection = Response<Body>;

            #[allow(non_snake_case)]
            async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
                let params = if let Some(params) = req
                    .extensions_mut()
                    .get_mut::<Option<crate::routing::UrlParams>>()
                {
                    params.take().expect("params already taken").0
                } else {
                    return Err(MissingRouteParams(()).into_response())
                };

                if let [(_, $head), $((_, $tail),)*] = &*params {
                    let $head = if let Ok(x) = $head.parse::<$head>() {
                       x
                    } else {
                        return Err(InvalidUrlParam::new::<$head>().into_response());
                    };

                    $(
                        let $tail = if let Ok(x) = $tail.parse::<$tail>() {
                           x
                        } else {
                            return Err(InvalidUrlParam::new::<$tail>().into_response());
                        };
                    )*

                    Ok(UrlParams(($head, $($tail,)*)))
                } else {
                    return Err(MissingRouteParams(()).into_response())
                }
            }
        }

        impl_parse_url!($($tail,)*);
    };
}

impl_parse_url!(T1, T2, T3, T4, T5, T6);

fn take_body(req: &mut Request<Body>) -> Result<Body, BodyAlreadyTaken> {
    struct BodyAlreadyTakenExt;

    if req.extensions_mut().insert(BodyAlreadyTakenExt).is_some() {
        Err(BodyAlreadyTaken(()))
    } else {
        let body = std::mem::take(req.body_mut());
        Ok(body)
    }
}
