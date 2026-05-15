use axum_core::__composite_rejection as composite_rejection;
use axum_core::__define_rejection as define_rejection;
use axum_core::extract::FromRequestParts;
use http::request::Parts;
use serde::de::DeserializeOwned;

fn deserialize_pair<T: DeserializeOwned>(key: &str, value: &str) -> Result<T, serde_json::Error> {
    if let Ok(n) = value.parse::<i64>() {
        serde_json::from_value(serde_json::json!({ key: n }))
    } else if let Ok(b) = value.parse::<bool>() {
        serde_json::from_value(serde_json::json!({ key: b }))
    } else {
        serde_json::from_value(serde_json::json!({ key: value }))
    }
}

/// Extractor that deserializes query string into `Vec<T>`.
/// Each key-value pair is deserialized into one `T`.
///
/// `MAX` limits the total number of items. Defaults to unlimited.
///
/// # Example
/// ```rust,no_run
/// use axum_extra::extract::QueryList;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// #[serde(rename_all = "lowercase")]
/// enum Filter {
///     Id(u32),
///     Username(String),
/// }
///
/// async fn handler(QueryList(filters): QueryList<Filter>) { }
/// async fn handler_capped(QueryList(filters): QueryList<Filter, 10>) { }
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "query")))]
#[derive(Debug, Clone, Default)]
pub struct QueryList<T, const MAX: usize = { usize::MAX }>(pub Vec<T>);

impl<T, S, const MAX: usize> FromRequestParts<S> for QueryList<T, MAX>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = QueryListRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or_default();

        let result = form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| {
                deserialize_pair::<T>(k.as_ref(), v.as_ref())
                    .map_err(FailedToDeserializeQueryString::from_err)
            })
            .collect::<Result<Vec<_>, _>>()?;

        if result.len() > MAX {
            return Err(FailedToDeserializeQueryString::from_err(format!(
                "too many query parameters, max allowed is {MAX}"
            ))
            .into());
        }

        Ok(Self(result))
    }
}

/// Extractor that deserializes query string into `Vec<T>`.
/// Returns empty `Vec` if no query parameters are present.
///
/// `MAX` limits the total number of items. Defaults to unlimited.
#[cfg_attr(docsrs, doc(cfg(feature = "query")))]
#[derive(Debug, Clone, Default)]
pub struct OptionalQueryList<T, const MAX: usize = { usize::MAX }>(pub Vec<T>);

impl<T, S, const MAX: usize> FromRequestParts<S> for OptionalQueryList<T, MAX>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = QueryListRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or_default();

        let result = form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| {
                deserialize_pair::<T>(k.as_ref(), v.as_ref())
                    .map_err(FailedToDeserializeQueryString::from_err)
            })
            .collect::<Result<Vec<_>, _>>()?;

        if result.len() > MAX {
            return Err(FailedToDeserializeQueryString::from_err(format!(
                "too many query parameters, max allowed is {MAX}"
            ))
            .into());
        }

        Ok(Self(result))
    }
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to deserialize query string"]
    pub struct FailedToDeserializeQueryString(Error);
}

composite_rejection! {
    pub enum QueryListRejection {
        FailedToDeserializeQueryString,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[allow(dead_code)]
    enum List {
        Id(u32),
        Username(String),
    }

    #[tokio::test]
    async fn test_query_list_enum() {
        let uri: http::Uri = "/?id=123&username=abc&id=345&username=def".parse().unwrap();
        let mut parts = http::Request::builder()
            .uri(uri)
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let result = QueryList::<List>::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_same_variant_repeated() {
        let uri: http::Uri = "/?id=1&id=2&id=3".parse().unwrap();
        let mut parts = http::Request::builder()
            .uri(uri)
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let result = QueryList::<List>::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.len(), 3);
    }

    #[tokio::test]
    async fn test_empty_query_string() {
        let uri: http::Uri = "/".parse().unwrap();
        let mut parts = http::Request::builder()
            .uri(uri)
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let result = OptionalQueryList::<List>::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.len(), 0);
    }

    #[tokio::test]
    async fn test_unknown_variant_returns_error() {
        let uri: http::Uri = "/?unknown=123".parse().unwrap();
        let mut parts = http::Request::builder()
            .uri(uri)
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let result = QueryList::<List>::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_value_returns_error() {
        let uri: http::Uri = "/?id=abc".parse().unwrap();
        let mut parts = http::Request::builder()
            .uri(uri)
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let result = QueryList::<List>::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_special_characters() {
        let uri: http::Uri = "/?username=hello%20world".parse().unwrap();
        let mut parts = http::Request::builder()
            .uri(uri)
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let result = QueryList::<List>::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cap_limit() {
        let uri: http::Uri = "/?id=1&id=2&id=3".parse().unwrap();
        let mut parts = http::Request::builder()
            .uri(uri)
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let result = QueryList::<List, 2>::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
    }
}
