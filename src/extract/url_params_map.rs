use super::{rejection::*, FromRequest, RequestParts};
use crate::util::ByteStr;
use async_trait::async_trait;
use std::{collections::HashMap, str::FromStr};

/// Extractor that will get captures from the URL.
///
/// # Example
///
/// ```rust,no_run
/// use axum::prelude::*;
///
/// async fn users_show(params: extract::UrlParamsMap) {
///     let id: Option<&str> = params.get("id");
///
///     // ...
/// }
///
/// let app = route("/users/:id", get(users_show));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Note that you can only have one URL params extractor per handler. If you
/// have multiple it'll response with `500 Internal Server Error`.
#[derive(Debug)]
#[deprecated(since = "0.1.3", note = "Use `axum::extract::Path` instead.")]
pub struct UrlParamsMap(HashMap<ByteStr, ByteStr>);

#[allow(deprecated)]
impl UrlParamsMap {
    /// Look up the value for a key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(&ByteStr::new(key)).map(|s| s.as_str())
    }

    /// Look up the value for a key and parse it into a value of type `T`.
    pub fn get_typed<T>(&self, key: &str) -> Option<Result<T, T::Err>>
    where
        T: FromStr,
    {
        self.get(key).map(str::parse)
    }
}

#[async_trait]
#[allow(deprecated)]
impl<B> FromRequest<B> for UrlParamsMap
where
    B: Send,
{
    type Rejection = MissingRouteParams;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if let Some(params) = req
            .extensions_mut()
            .and_then(|ext| ext.get_mut::<Option<crate::routing::UrlParams>>())
        {
            if let Some(params) = params {
                Ok(Self(params.0.iter().cloned().collect()))
            } else {
                Ok(Self(Default::default()))
            }
        } else {
            Err(MissingRouteParams)
        }
    }
}
