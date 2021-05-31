use crate::{body::Body, Error};
use async_trait::async_trait;
use bytes::Bytes;
use http::{header, Request, StatusCode};
use serde::de::DeserializeOwned;
use std::{collections::HashMap, str::FromStr};

#[async_trait]
pub trait FromRequest: Sized {
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Error>;
}

fn take_body(req: &mut Request<Body>) -> Body {
    struct BodyAlreadyTaken;

    if req.extensions_mut().insert(BodyAlreadyTaken).is_some() {
        panic!("Cannot have two request body on extractors")
    } else {
        let body = std::mem::take(req.body_mut());
        body
    }
}

#[async_trait]
impl<T> FromRequest for Option<T>
where
    T: FromRequest,
{
    async fn from_request(req: &mut Request<Body>) -> Result<Option<T>, Error> {
        Ok(T::from_request(req).await.ok())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Query<T>(T);

impl<T> Query<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

#[async_trait]
impl<T> FromRequest for Query<T>
where
    T: DeserializeOwned,
{
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Error> {
        let query = req.uri().query().ok_or(Error::QueryStringMissing)?;
        let value = serde_urlencoded::from_str(query).map_err(Error::DeserializeQueryString)?;
        Ok(Query(value))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Json<T>(T);

impl<T> Json<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

#[async_trait]
impl<T> FromRequest for Json<T>
where
    T: DeserializeOwned,
{
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Error> {
        if has_content_type(&req, "application/json") {
            let body = take_body(req);

            let bytes = hyper::body::to_bytes(body)
                .await
                .map_err(Error::ConsumeRequestBody)?;
            let value = serde_json::from_slice(&bytes).map_err(Error::DeserializeRequestBody)?;
            Ok(Json(value))
        } else {
            Err(Error::Status(StatusCode::BAD_REQUEST))
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
pub struct Extension<T>(T);

impl<T> Extension<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

#[async_trait]
impl<T> FromRequest for Extension<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Error> {
        let value = req
            .extensions()
            .get::<T>()
            .ok_or_else(|| Error::MissingExtension {
                type_name: std::any::type_name::<T>(),
            })
            .map(|x| x.clone())?;

        Ok(Extension(value))
    }
}

#[async_trait]
impl FromRequest for Bytes {
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Error> {
        let body = take_body(req);

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(Error::ConsumeRequestBody)?;

        Ok(bytes)
    }
}

#[async_trait]
impl FromRequest for String {
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Error> {
        let body = take_body(req);

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(Error::ConsumeRequestBody)?
            .to_vec();

        let string = String::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;

        Ok(string)
    }
}

#[async_trait]
impl FromRequest for Body {
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Error> {
        let body = take_body(req);
        Ok(body)
    }
}

#[derive(Debug, Clone)]
pub struct BytesMaxLength<const N: u64>(Bytes);

impl<const N: u64> BytesMaxLength<N> {
    pub fn into_inner(self) -> Bytes {
        self.0
    }
}

#[async_trait]
impl<const N: u64> FromRequest for BytesMaxLength<N> {
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Error> {
        let content_length = req.headers().get(http::header::CONTENT_LENGTH).cloned();
        let body = take_body(req);

        let content_length =
            content_length.and_then(|value| value.to_str().ok()?.parse::<u64>().ok());

        if let Some(length) = content_length {
            if length > N {
                return Err(Error::PayloadTooLarge);
            }
        } else {
            return Err(Error::LengthRequired);
        };

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(Error::ConsumeRequestBody)?;

        Ok(BytesMaxLength(bytes))
    }
}

pub struct UrlParamsMap(HashMap<String, String>);

impl UrlParamsMap {
    pub fn get(&self, key: &str) -> Result<&str, Error> {
        if let Some(value) = self.0.get(key) {
            Ok(value)
        } else {
            Err(Error::UnknownUrlParam(key.to_string()))
        }
    }

    pub fn get_typed<T>(&self, key: &str) -> Result<T, Error>
    where
        T: FromStr,
    {
        self.get(key)?.parse().map_err(|_| Error::InvalidUrlParam {
            type_name: std::any::type_name::<T>(),
        })
    }
}

#[async_trait]
impl FromRequest for UrlParamsMap {
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Error> {
        if let Some(params) = req
            .extensions_mut()
            .get_mut::<Option<crate::routing::UrlParams>>()
        {
            let params = params.take().expect("params already taken").0;
            Ok(Self(params.into_iter().collect()))
        } else {
            panic!("no url params found for matched route. This is a bug in tower-web")
        }
    }
}

pub struct UrlParams<T>(T);

impl<T> UrlParams<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

macro_rules! impl_parse_url {
    () => {};

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        impl<$head, $($tail,)*> FromRequest for UrlParams<($head, $($tail,)*)>
        where
            $head: FromStr + Send,
            $( $tail: FromStr + Send, )*
        {
            #[allow(non_snake_case)]
            async fn from_request(req: &mut Request<Body>) -> Result<Self, Error> {
                let params = if let Some(params) = req
                    .extensions_mut()
                    .get_mut::<Option<crate::routing::UrlParams>>()
                {
                    params.take().expect("params already taken").0
                } else {
                    panic!("no url params found for matched route. This is a bug in tower-web")
                };

                if let [(_, $head), $((_, $tail),)*] = &*params {
                    let $head = if let Ok(x) = $head.parse::<$head>() {
                       x
                    } else {
                        return Err(Error::InvalidUrlParam {
                            type_name: std::any::type_name::<$head>(),
                        });
                    };

                    $(
                        let $tail = if let Ok(x) = $tail.parse::<$tail>() {
                           x
                        } else {
                            return Err(Error::InvalidUrlParam {
                                type_name: std::any::type_name::<$tail>(),
                            });
                        };
                    )*

                    Ok(UrlParams(($head, $($tail,)*)))
                } else {
                    panic!("wrong number of url params found for matched route. This is a bug in tower-web")
                }
            }
        }

        impl_parse_url!($($tail,)*);
    };
}

impl_parse_url!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);
