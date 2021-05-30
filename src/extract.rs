use crate::{body::Body, Error};
use bytes::Bytes;
use futures_util::{future, ready};
use http::Request;
use http_body::Body as _;
use pin_project::pin_project;
use serde::de::DeserializeOwned;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    str::FromStr,
    task::{Context, Poll},
};

pub trait FromRequest: Sized {
    type Future: Future<Output = Result<Self, Error>> + Send;

    fn from_request(req: &mut Request<Body>) -> Self::Future;
}

impl<T> FromRequest for Option<T>
where
    T: FromRequest,
{
    type Future = OptionFromRequestFuture<T::Future>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        OptionFromRequestFuture(T::from_request(req))
    }
}

#[pin_project]
pub struct OptionFromRequestFuture<F>(#[pin] F);

impl<F, T> Future for OptionFromRequestFuture<F>
where
    F: Future<Output = Result<T, Error>>,
{
    type Output = Result<Option<T>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let value = ready!(self.project().0.poll(cx));
        Poll::Ready(Ok(value.ok()))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Query<T>(T);

impl<T> Query<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> FromRequest for Query<T>
where
    T: DeserializeOwned + Send,
{
    type Future = future::Ready<Result<Self, Error>>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        let result = (|| {
            let query = req.uri().query().ok_or(Error::QueryStringMissing)?;
            let value = serde_urlencoded::from_str(query).map_err(Error::DeserializeQueryString)?;
            Ok(Query(value))
        })();

        future::ready(result)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Json<T>(T);

impl<T> Json<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> FromRequest for Json<T>
where
    T: DeserializeOwned,
{
    type Future = future::BoxFuture<'static, Result<Self, Error>>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        // TODO(david): require the body to have `content-type: application/json`

        let body = std::mem::take(req.body_mut());

        Box::pin(async move {
            let bytes = hyper::body::to_bytes(body)
                .await
                .map_err(Error::ConsumeRequestBody)?;
            let value = serde_json::from_slice(&bytes).map_err(Error::DeserializeRequestBody)?;
            Ok(Json(value))
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Extension<T>(T);

impl<T> Extension<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> FromRequest for Extension<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Future = future::Ready<Result<Self, Error>>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        let result = (|| {
            let value = req
                .extensions()
                .get::<T>()
                .ok_or_else(|| Error::MissingExtension {
                    type_name: std::any::type_name::<T>(),
                })
                .map(|x| x.clone())?;
            Ok(Extension(value))
        })();

        future::ready(result)
    }
}

impl FromRequest for Bytes {
    type Future = future::BoxFuture<'static, Result<Self, Error>>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        let body = std::mem::take(req.body_mut());

        Box::pin(async move {
            let bytes = hyper::body::to_bytes(body)
                .await
                .map_err(Error::ConsumeRequestBody)?;
            Ok(bytes)
        })
    }
}

#[derive(Debug, Clone)]
pub struct BytesMaxLength<const N: u64>(Bytes);

impl<const N: u64> BytesMaxLength<N> {
    pub fn into_inner(self) -> Bytes {
        self.0
    }
}

impl<const N: u64> FromRequest for BytesMaxLength<N> {
    type Future = future::BoxFuture<'static, Result<Self, Error>>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        let content_length = req.headers().get(http::header::CONTENT_LENGTH).cloned();
        let body = std::mem::take(req.body_mut());

        Box::pin(async move {
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
        })
    }
}

pub struct UrlParams(HashMap<String, String>);

impl UrlParams {
    pub fn get(&self, key: &str) -> Result<&str, Error> {
        if let Some(value) = self.0.get(key) {
            Ok(value)
        } else {
            Err(Error::UnknownUrlParam(key.to_string()))
        }
    }
}

impl FromRequest for UrlParams {
    type Future = future::Ready<Result<Self, Error>>;

    fn from_request(req: &mut Request<Body>) -> Self::Future {
        if let Some(params) = req
            .extensions_mut()
            .get_mut::<Option<crate::routing::UrlParams>>()
        {
            let params = params.take().expect("params already taken").0;
            future::ok(Self(params.into_iter().collect()))
        } else {
            panic!("no url params found for matched route. This is a bug in tower-web")
        }
    }
}
