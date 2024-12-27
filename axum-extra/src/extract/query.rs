use axum::extract::FromRequestParts;
use axum_core::__composite_rejection as composite_rejection;
use axum_core::__define_rejection as define_rejection;
use http::request::Parts;
use serde::de::DeserializeOwned;

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
        let deserializer =
            serde_html_form::Deserializer::new(form_urlencoded::parse(query.as_bytes()));
        let value = serde_path_to_error::deserialize(deserializer)
            .map_err(FailedToDeserializeQueryString::from_err)?;
        Ok(Query(value))
    }
}

axum_core::__impl_deref!(Query);

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to deserialize query string"]
    /// Rejection type used if the [`Query`] extractor is unable to
    /// deserialize the query string into the target type.
    pub struct FailedToDeserializeQueryString(Error);
}

composite_rejection! {
    /// Rejection used for [`Query`].
    ///
    /// Contains one variant for each way the [`Query`] extractor can fail.
    pub enum QueryRejection {
        FailedToDeserializeQueryString,
    }
}

/// Extractor that deserializes query strings into `None` if no query parameters are present.
///
/// Otherwise behaviour is identical to [`Query`].
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
#[derive(Debug, Clone, Copy, Default)]
pub struct OptionalQuery<T>(pub Option<T>);

impl<T, S> FromRequestParts<S> for OptionalQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = OptionalQueryRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(query) = parts.uri.query() {
            let deserializer =
                serde_html_form::Deserializer::new(form_urlencoded::parse(query.as_bytes()));
            let value = serde_path_to_error::deserialize(deserializer)
                .map_err(FailedToDeserializeQueryString::from_err)?;
            Ok(OptionalQuery(Some(value)))
        } else {
            Ok(OptionalQuery(None))
        }
    }
}

impl<T> std::ops::Deref for OptionalQuery<T> {
    type Target = Option<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for OptionalQuery<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

composite_rejection! {
    /// Rejection used for [`OptionalQuery`].
    ///
    /// Contains one variant for each way the [`OptionalQuery`] extractor can fail.
    pub enum OptionalQueryRejection {
        FailedToDeserializeQueryString,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::routing::{get, post};
    use axum::Router;
    use http::header::CONTENT_TYPE;
    use http::StatusCode;
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
    async fn correct_rejection_status_code() {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct Params {
            n: i32,
        }

        async fn handler(_: Query<Params>) {}

        let app = Router::new().route("/", get(handler));
        let client = TestClient::new(app);

        let res = client.get("/?n=hi").await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            res.text().await,
            "Failed to deserialize query string: n: invalid digit found in string"
        );
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
