use axum::{
    extract::{FromRequestParts, OptionalFromRequestParts},
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
/// # `Option<Query<T>>` behavior
///
/// If `Query<T>` itself is used as an extractor and there is no query string in
/// the request URL, `T`'s `Deserialize` implementation is called on an empty
/// string instead.
///
/// You can avoid this by using `Option<Query<T>>`, which gives you `None` in
/// the case that there is no query string in the request URL.
///
/// Note that an empty query string is not the same as no query string, that is
/// `https://example.org/` and `https://example.org/?` are not treated the same
/// in this case.
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
/// # let _: Router = app;
/// ```
///
/// If the query string cannot be parsed it will reject the request with a `400
/// Bad Request` response.
///
/// For handling values being empty vs missing see the [query-params-with-empty-strings][example]
/// example.
///
/// [example]: https://github.com/tokio-rs/axum/blob/main/examples/query-params-with-empty-strings/src/main.rs
///
/// While `Option<T>` will handle empty parameters (e.g. `param=`), beware when using this with a
/// `Vec<T>`. If your list is optional, use `Vec<T>` in combination with `#[serde(default)]`
/// instead of `Option<Vec<T>>`. `Option<Vec<T>>` will handle 0, 2, or more arguments, but not one
/// argument.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{routing::get, Router};
/// use axum_extra::extract::Query;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Params {
///     #[serde(default)]
///     items: Vec<usize>,
/// }
///
/// // This will parse 0 occurrences of `items` as an empty `Vec`.
/// async fn process_items(Query(params): Query<Params>) {
///     // ...
/// }
///
/// let app = Router::new().route("/process_items", get(process_items));
/// # let _: Router = app;
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "query")))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

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

impl<T, S> OptionalFromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = QueryRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        if let Some(query) = parts.uri.query() {
            let value = serde_html_form::from_str(query)
                .map_err(|err| QueryRejection::FailedToDeserializeQueryString(Error::new(err)))?;
            Ok(Some(Self(value)))
        } else {
            Ok(None)
        }
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
            Self::FailedToDeserializeQueryString(inner) => {
                let body = format!("Failed to deserialize query string: {inner}");
                let status = StatusCode::BAD_REQUEST;
                axum_core::__log_rejection!(
                    rejection_type = Self,
                    body_text = body,
                    status = status,
                );
                (status, body).into_response()
            }
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

/// Extractor that deserializes query strings into `None` if no query parameters are present.
/// Otherwise behaviour is identical to [`Query`]
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Example
///
/// ```rust,no_run
/// use axum::{routing::get, Router};
/// use axum_extra::extract::OptionalQuery;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Pagination {
///     page: usize,
///     per_page: usize,
/// }
///
/// // This will parse query strings like `?page=2&per_page=30` into `Some(Pagination)` and
/// // empty query string into `None`
/// async fn list_things(OptionalQuery(pagination): OptionalQuery<Pagination>) {
///     match pagination {
///         Some(Pagination{ page, per_page }) => { /* return specified page */ },
///         None => { /* return fist page */ }
///     }
///     // ...
/// }
///
/// let app = Router::new().route("/list_things", get(list_things));
/// # let _: Router = app;
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
#[deprecated = "Use Option<Query<_>> instead"]
#[derive(Debug, Clone, Copy, Default)]
pub struct OptionalQuery<T>(pub Option<T>);

#[allow(deprecated)]
impl<T, S> FromRequestParts<S> for OptionalQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = OptionalQueryRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(query) = parts.uri.query() {
            let value = serde_html_form::from_str(query).map_err(|err| {
                OptionalQueryRejection::FailedToDeserializeQueryString(Error::new(err))
            })?;
            Ok(OptionalQuery(Some(value)))
        } else {
            Ok(OptionalQuery(None))
        }
    }
}

#[allow(deprecated)]
impl<T> std::ops::Deref for OptionalQuery<T> {
    type Target = Option<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[allow(deprecated)]
impl<T> std::ops::DerefMut for OptionalQuery<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Rejection used for [`OptionalQuery`].
///
/// Contains one variant for each way the [`OptionalQuery`] extractor can fail.
#[derive(Debug)]
#[non_exhaustive]
#[cfg(feature = "query")]
pub enum OptionalQueryRejection {
    #[allow(missing_docs)]
    FailedToDeserializeQueryString(Error),
}

impl IntoResponse for OptionalQueryRejection {
    fn into_response(self) -> Response {
        match self {
            Self::FailedToDeserializeQueryString(inner) => (
                StatusCode::BAD_REQUEST,
                format!("Failed to deserialize query string: {inner}"),
            )
                .into_response(),
        }
    }
}

impl fmt::Display for OptionalQueryRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FailedToDeserializeQueryString(inner) => inner.fmt(f),
        }
    }
}

impl std::error::Error for OptionalQueryRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FailedToDeserializeQueryString(inner) => Some(inner),
        }
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{routing::post, Router};
    use http::header::CONTENT_TYPE;
    use serde::Deserialize;

    #[tokio::test]
    async fn query_supports_multiple_values() {
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
            .await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "one,two");
    }

    #[tokio::test]
    async fn optional_query_supports_multiple_values() {
        #[derive(Deserialize)]
        struct Data {
            #[serde(rename = "value")]
            values: Vec<String>,
        }

        let app = Router::new().route(
            "/",
            post(|OptionalQuery(data): OptionalQuery<Data>| async move {
                data.map(|Data { values }| values.join(","))
                    .unwrap_or("None".to_owned())
            }),
        );

        let client = TestClient::new(app);

        let res = client
            .post("/?value=one&value=two")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body("")
            .await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "one,two");
    }

    #[tokio::test]
    async fn optional_query_deserializes_no_parameters_into_none() {
        #[derive(Deserialize)]
        struct Data {
            value: String,
        }

        let app = Router::new().route(
            "/",
            post(|OptionalQuery(data): OptionalQuery<Data>| async move {
                match data {
                    None => "None".into(),
                    Some(data) => data.value,
                }
            }),
        );

        let client = TestClient::new(app);

        let res = client.post("/").body("").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "None");
    }

    #[tokio::test]
    async fn optional_query_preserves_parsing_errors() {
        #[derive(Deserialize)]
        struct Data {
            value: String,
        }

        let app = Router::new().route(
            "/",
            post(|OptionalQuery(data): OptionalQuery<Data>| async move {
                match data {
                    None => "None".into(),
                    Some(data) => data.value,
                }
            }),
        );

        let client = TestClient::new(app);

        let res = client
            .post("/?other=something")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body("")
            .await;

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
