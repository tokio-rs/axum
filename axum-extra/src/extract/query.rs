use axum::{
    async_trait,
    extract::FromRequestParts,
    response::{IntoResponse, Response},
    Error,
};
use http::{request::Parts, StatusCode};
use serde::de::DeserializeOwned;
use std::fmt;

/// Extractor that deserializes query strings into some type.
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Differences from `axum::extract::Query`
///
/// This extractor uses [`serde_html_form`] under-the-hood which supports multi-value items. These
/// are sent by multiple `<input>` attributes of the same name (e.g. checkboxes) and `<select>`s
/// with the `multiple` attribute. Those values can be collected into a `Vec` or other sequential
/// container.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{routing::get, Router};
/// use axum_extra::extract::Query;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Pagination {
///     page: usize,
///     per_page: usize,
/// }
///
/// // This will parse query strings like `?page=2&per_page=30` into `Pagination`
/// // structs.
/// async fn list_things(pagination: Query<Pagination>) {
///     let pagination: Pagination = pagination.0;
///
///     // ...
/// }
///
/// let app = Router::new().route("/list_things", get(list_things));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If the query string cannot be parsed it will reject the request with a `400
/// Bad Request` response.
///
/// For handling values being empty vs missing see the [query-params-with-empty-strings][example]
/// example.
///
/// [example]: https://github.com/tokio-rs/axum/blob/main/examples/query-params-with-empty-strings/src/main.rs
#[cfg_attr(docsrs, doc(cfg(feature = "query")))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

#[async_trait]
impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = QueryRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or_default();
        let value = serde_html_form::from_str(query)
            .map_err(|err| QueryRejection::FailedToDeserializeQueryString(Error::new(err)))?;
        Ok(Query(value))
    }
}

axum_core::__impl_deref!(Query);

/// Rejection used for [`Query`].
///
/// Contains one variant for each way the [`Query`] extractor can fail.
#[derive(Debug)]
#[non_exhaustive]
#[cfg(feature = "query")]
pub enum QueryRejection {
    #[allow(missing_docs)]
    FailedToDeserializeQueryString(Error),
}

impl IntoResponse for QueryRejection {
    fn into_response(self) -> Response {
        match self {
            Self::FailedToDeserializeQueryString(inner) => (
                StatusCode::BAD_REQUEST,
                format!("Failed to deserialize query string: {}", inner),
            )
                .into_response(),
        }
    }
}

impl fmt::Display for QueryRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FailedToDeserializeQueryString(inner) => inner.fmt(f),
        }
    }
}

impl std::error::Error for QueryRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FailedToDeserializeQueryString(inner) => Some(inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{routing::post, Router};
    use http::{header::CONTENT_TYPE, StatusCode};
    use serde::Deserialize;

    #[tokio::test]
    async fn supports_multiple_values() {
        #[derive(Deserialize)]
        struct Data {
            #[serde(rename = "value")]
            values: Vec<String>,
        }

        let app = Router::new().route(
            "/",
            post(|Query(data): Query<Data>| async move { data.values.join(",") }),
        );

        let client = TestClient::new(app);

        let res = client
            .post("/?value=one&value=two")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body("")
            .send()
            .await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "one,two");
    }
}
