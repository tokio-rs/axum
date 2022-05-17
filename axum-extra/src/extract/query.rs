use axum::{
    async_trait,
    extract::{
        rejection::{FailedToDeserializeQueryString, QueryRejection},
        FromRequest, RequestParts,
    },
};
use serde::de::DeserializeOwned;
use std::ops::Deref;

/// Extractor that deserializes query strings into some type.
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Differences from `axum::extract::Form`
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
/// If the query string cannot be parsed it will reject the request with a `422
/// Unprocessable Entity` response.
///
/// For handling values being empty vs missing see the (query-params-with-empty-strings)[example]
/// example.
///
/// [example]: https://github.com/tokio-rs/axum/blob/main/examples/query-params-with-empty-strings/src/main.rs
#[cfg_attr(docsrs, doc(cfg(feature = "query")))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for Query<T>
where
    T: DeserializeOwned,
    B: Send,
{
    type Rejection = QueryRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let query = req.uri().query().unwrap_or_default();
        let value = serde_html_form::from_str(query)
            .map_err(FailedToDeserializeQueryString::__private_new::<T, _>)?;
        Ok(Query(value))
    }
}

impl<T> Deref for Query<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
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
