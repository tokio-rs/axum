#![allow(deprecated)]

use axum_core::__composite_rejection as composite_rejection;
use axum_core::__define_rejection as define_rejection;
use axum_core::extract::FromRequestParts;
use http::{request::Parts, Uri};
use serde_core::de::DeserializeOwned;

/// Extractor that deserializes query strings into some type.
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Deprecated
///
/// This extractor used to use a different deserializer under-the-hood but that
/// is no longer the case. Now it only uses an older version of the same
/// deserializer, purely for ease of transition to the latest version.
/// Before switching to `axum::extract::Form`, it is recommended to read the
/// [changelog for `serde_html_form v0.3.0`][changelog].
///
/// [changelog]: https://github.com/jplatte/serde_html_form/blob/main/CHANGELOG.md#030
#[deprecated = "see documentation"]
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
        Ok(Self(value))
    }
}

impl<T> Query<T>
where
    T: DeserializeOwned,
{
    /// Attempts to construct a [`Query`] from a reference to a [`Uri`].
    ///
    /// # Example
    /// ```
    /// use axum_extra::extract::Query;
    /// use http::Uri;
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct ExampleParams {
    ///     foo: String,
    ///     bar: u32,
    /// }
    ///
    /// let uri: Uri = "http://example.com/path?foo=hello&bar=42".parse().unwrap();
    /// let result: Query<ExampleParams> = Query::try_from_uri(&uri).unwrap();
    /// assert_eq!(result.foo, String::from("hello"));
    /// assert_eq!(result.bar, 42);
    /// ```
    pub fn try_from_uri(value: &Uri) -> Result<Self, QueryRejection> {
        let query = value.query().unwrap_or_default();
        let params =
            serde_html_form::from_str(query).map_err(FailedToDeserializeQueryString::from_err)?;
        Ok(Self(params))
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
    #[deprecated = "because Query is deprecated"]
    pub enum QueryRejection {
        FailedToDeserializeQueryString,
    }
}

/// Extractor that deserializes query strings into `None` if no query parameters are present.
///
/// Otherwise behaviour is identical to [`Query`][axum::extract::Query].
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
            Ok(Self(Some(value)))
        } else {
            Ok(Self(None))
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

fn deserialize_pair<T: DeserializeOwned>(
    key: String,
    value: String,
) -> Result<T, serde_json::Error> {
    let mut map = serde_json::Map::new();
    let parsed_value: serde_json::Value = match value.parse::<serde_json::Number>() {
        Ok(num) => serde_json::Value::Number(num),
        Err(_) => match value.as_str() {
            "true" => serde_json::Value::Bool(true),
            "false" => serde_json::Value::Bool(false),
            "null" => serde_json::Value::Null,
            _ => serde_json::Value::String(value),
        },
    };
    map.insert(key, parsed_value);
    serde_json::from_value(serde_json::Value::Object(map))
}

/// Extractor that deserializes query strings into `Vec<T>` where each query parameter
/// becomes an enum variant.
///
/// This is useful for deserializing alternating query parameters of different types,
/// like `?id=123&username=abc&id=456` into `Vec<List>` where `List` is an enum with
/// `Id(u32)` and `Username(String)` variants.
///
/// `T` is expected to be an enum that implements the `Deserialize` trait with externally-tagged
/// variants where each variant name matches a query parameter name.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{routing::get, Router};
/// use axum_extra::extract::QueryList;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// #[serde(rename_all = "lowercase")]
/// enum Param {
///     Id(u32),
///     Username(String),
/// }
///
/// async fn handler(QueryList(params): QueryList<Param>) {
///     for param in params {
///         match param {
///             Param::Id(id) => println!("ID: {}", id),
///             Param::Username(name) => println!("Username: {}", name),
///         }
///     }
/// }
///
/// let app = Router::new().route("/", get(handler));
/// # let _: Router = app;
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "query")))]
#[derive(Debug, Clone, Default)]
pub struct QueryList<T>(pub Vec<T>);

impl<T, S> FromRequestParts<S> for QueryList<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = QueryListRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or_default();
        let pairs: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        let mut result = Vec::new();
        for (key, value) in pairs {
            let item = deserialize_pair::<T>(key, value)
                .map_err(FailedToDeserializeQueryString::from_err)?;
            result.push(item);
        }

        Ok(Self(result))
    }
}

impl<T> QueryList<T>
where
    T: DeserializeOwned,
{
    /// Attempts to construct a [`QueryList`] from a reference to a [`Uri`].
    pub fn try_from_uri(value: &Uri) -> Result<Self, QueryListRejection> {
        let query = value.query().unwrap_or_default();
        let pairs: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        let mut result = Vec::new();
        for (key, value) in pairs {
            let item = deserialize_pair::<T>(key, value)
                .map_err(FailedToDeserializeQueryString::from_err)?;
            result.push(item);
        }

        Ok(Self(result))
    }
}

impl<T> std::ops::Deref for QueryList<T> {
    type Target = Vec<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for QueryList<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

composite_rejection! {
    /// Rejection used for [`QueryList`].
    ///
    /// Contains one variant for each way the [`QueryList`] extractor can fail.
    pub enum QueryListRejection {
        FailedToDeserializeQueryString,
    }
}

/// Extractor that deserializes query strings into `Vec<T>` when parameters are present,
/// or an empty `Vec` if no query parameters exist.
///
/// Otherwise behaviour is identical to [`QueryList`].
/// `T` is expected to be an enum that implements the `Deserialize` trait.
#[cfg_attr(docsrs, doc(cfg(feature = "query")))]
#[derive(Debug, Clone, Default)]
pub struct OptionalQueryList<T>(pub Vec<T>);

impl<T, S> FromRequestParts<S> for OptionalQueryList<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = OptionalQueryListRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(query) = parts.uri.query() {
            let pairs: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();

            let mut result = Vec::new();
            for (key, value) in pairs {
                let item = deserialize_pair::<T>(key, value)
                    .map_err(FailedToDeserializeQueryString::from_err)?;
                result.push(item);
            }

            Ok(Self(result))
        } else {
            Ok(Self(Vec::new()))
        }
    }
}

impl<T> std::ops::Deref for OptionalQueryList<T> {
    type Target = Vec<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for OptionalQueryList<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

composite_rejection! {
    /// Rejection used for [`OptionalQueryList`].
    ///
    /// Contains one variant for each way the [`OptionalQueryList`] extractor can fail.
    pub enum OptionalQueryListRejection {
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
        #[allow(dead_code)]
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
        #[allow(dead_code)]
        #[derive(Deserialize)]
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
        #[allow(dead_code)]
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
        #[allow(dead_code)]
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
        #[allow(dead_code)]
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

    #[test]
    fn test_try_from_uri() {
        #[allow(dead_code)]
        #[derive(Deserialize)]
        struct TestQueryParams {
            foo: Vec<String>,
            bar: u32,
        }
        let uri: Uri = "http://example.com/path?foo=hello&bar=42&foo=goodbye"
            .parse()
            .unwrap();
        let result: Query<TestQueryParams> = Query::try_from_uri(&uri).unwrap();
        assert_eq!(result.foo, [String::from("hello"), String::from("goodbye")]);
        assert_eq!(result.bar, 42);
    }

    #[test]
    fn test_try_from_uri_with_invalid_query() {
        #[allow(dead_code)]
        #[derive(Deserialize)]
        struct TestQueryParams {
            _foo: String,
            _bar: u32,
        }
        let uri: Uri = "http://example.com/path?foo=hello&bar=invalid"
            .parse()
            .unwrap();
        let result: Result<Query<TestQueryParams>, _> = Query::try_from_uri(&uri);

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn query_list_deserializes_alternating_enum_variants() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Id(u32),
            Username(String),
        }

        let app = Router::new().route(
            "/",
            get(|QueryList(params): QueryList<Param>| async move {
                params
                    .iter()
                    .map(|p| match p {
                        Param::Id(id) => format!("id:{id}"),
                        Param::Username(u) => format!("user:{u}"),
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            }),
        );

        let client = TestClient::new(app);

        let res = client.get("/?id=123&username=alice&id=456").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "id:123|user:alice|id:456");
    }

    #[tokio::test]
    async fn query_list_maintains_order_with_repeated_variants() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Id(u32),
            Name(String),
        }

        let app = Router::new().route(
            "/",
            get(|QueryList(params): QueryList<Param>| async move { format!("{}", params.len()) }),
        );

        let client = TestClient::new(app);

        let res = client.get("/?id=1&name=a&id=2&name=b&id=3").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "5");
    }

    #[tokio::test]
    async fn query_list_empty_when_no_params() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize)]
        enum Param {
            Id(u32),
        }

        let app = Router::new().route(
            "/",
            get(|QueryList(params): QueryList<Param>| async move { format!("{}", params.len()) }),
        );

        let client = TestClient::new(app);

        let res = client.get("/").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "0");
    }

    #[tokio::test]
    async fn query_list_rejects_unknown_variant() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Id(u32),
        }

        let app = Router::new().route("/", get(|_: QueryList<Param>| async move { "ok" }));

        let client = TestClient::new(app);

        let res = client.get("/?unknown=value").await;

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn query_list_rejects_invalid_type_in_variant() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize)]
        enum Param {
            Id(u32),
        }

        let app = Router::new().route("/", get(|_: QueryList<Param>| async move { "ok" }));

        let client = TestClient::new(app);

        let res = client.get("/?id=not_a_number").await;

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn optional_query_list_returns_empty_when_no_params() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize)]
        enum Param {
            Id(u32),
        }

        let app = Router::new().route(
            "/",
            get(
                |OptionalQueryList(params): OptionalQueryList<Param>| async move {
                    format!("{}", params.len())
                },
            ),
        );

        let client = TestClient::new(app);

        let res = client.get("/").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "0");
    }

    #[tokio::test]
    async fn optional_query_list_deserializes_params_when_present() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Id(u32),
            Name(String),
        }

        let app = Router::new().route(
            "/",
            get(
                |OptionalQueryList(params): OptionalQueryList<Param>| async move {
                    format!("{}", params.len())
                },
            ),
        );

        let client = TestClient::new(app);

        let res = client.get("/?id=1&name=test").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "2");
    }

    #[tokio::test]
    async fn optional_query_list_rejects_invalid_param() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Id(u32),
        }

        let app = Router::new().route("/", get(|_: OptionalQueryList<Param>| async move { "ok" }));

        let client = TestClient::new(app);

        let res = client.get("/?id=invalid").await;

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_query_list_try_from_uri() {
        #[allow(dead_code)]
        #[derive(Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Id(u32),
            Name(String),
        }

        let uri: Uri = "http://example.com/path?id=123&name=alice&id=456"
            .parse()
            .unwrap();
        let result: QueryList<Param> = QueryList::try_from_uri(&uri).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_query_list_try_from_uri_with_invalid_param() {
        #[allow(dead_code)]
        #[derive(Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Id(u32),
        }

        let uri: Uri = "http://example.com/path?id=invalid".parse().unwrap();
        let result: Result<QueryList<Param>, _> = QueryList::try_from_uri(&uri);

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn query_list_decodes_url_encoded_whitespace() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Name(String),
            Tag(String),
        }

        let app = Router::new().route(
            "/",
            get(|QueryList(params): QueryList<Param>| async move {
                params
                    .iter()
                    .map(|p| match p {
                        Param::Name(n) => format!("name:{n}"),
                        Param::Tag(t) => format!("tag:{t}"),
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            }),
        );

        let client = TestClient::new(app);

        let res = client.get("/?name=john%20doe&tag=hello%20world").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "name:john doe|tag:hello world");

        let res = client.get("/?name=john+doe&tag=hello+world").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "name:john doe|tag:hello world");
    }

    #[tokio::test]
    async fn query_list_decodes_url_encoded_ampersand() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Name(String),
        }

        let app = Router::new().route(
            "/",
            get(|QueryList(params): QueryList<Param>| async move {
                params
                    .iter()
                    .map(|p| match p {
                        Param::Name(n) => format!("name:{n}"),
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            }),
        );

        let client = TestClient::new(app);

        let res = client.get("/?name=alice%26bob&name=charlie%26dave").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "name:alice&bob|name:charlie&dave");
    }

    #[tokio::test]
    async fn query_list_decodes_url_encoded_equals_sign() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Expression(String),
        }

        let app = Router::new().route(
            "/",
            get(|QueryList(params): QueryList<Param>| async move {
                params
                    .iter()
                    .map(|p| match p {
                        Param::Expression(e) => format!("expr:{e}"),
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            }),
        );

        let client = TestClient::new(app);

        let res = client.get("/?expression=x%3D5&expression=y%3D10").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "expr:x=5|expr:y=10");
    }

    #[tokio::test]
    async fn query_list_decodes_url_encoded_plus_sign() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Math(String),
        }

        let app = Router::new().route(
            "/",
            get(|QueryList(params): QueryList<Param>| async move {
                params
                    .iter()
                    .map(|p| match p {
                        Param::Math(m) => format!("math:{m}"),
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            }),
        );

        let client = TestClient::new(app);

        let res = client.get("/?math=1%2B2&math=3%2B4").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "math:1+2|math:3+4");
    }

    #[tokio::test]
    async fn query_list_decodes_url_encoded_special_characters() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Url(String),
            Path(String),
            Percent(String),
        }

        let app = Router::new().route(
            "/",
            get(|QueryList(params): QueryList<Param>| async move {
                params
                    .iter()
                    .map(|p| match p {
                        Param::Url(u) => format!("url:{u}"),
                        Param::Path(p) => format!("path:{p}"),
                        Param::Percent(p) => format!("pct:{p}"),
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            }),
        );

        let client = TestClient::new(app);

        let res = client
            .get("/?url=https%3A%2F%2Fexample.com&path=foo%2Fbar&percent=100%25")
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.text().await,
            "url:https://example.com|path:foo/bar|pct:100%"
        );
    }

    #[tokio::test]
    async fn query_list_handles_mixed_encoded_and_plain_text() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Name(String),
        }

        let app = Router::new().route(
            "/",
            get(|QueryList(params): QueryList<Param>| async move {
                params
                    .iter()
                    .map(|p| match p {
                        Param::Name(n) => format!("name:{n}"),
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            }),
        );

        let client = TestClient::new(app);

        let res = client
            .get("/?name=simple&name=with%20space&name=with%26ampersand")
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.text().await,
            "name:simple|name:with space|name:with&ampersand"
        );
    }

    #[tokio::test]
    async fn optional_query_list_decodes_url_encoded_special_characters() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Name(String),
            Tag(String),
        }

        let app = Router::new().route(
            "/",
            get(
                |OptionalQueryList(params): OptionalQueryList<Param>| async move {
                    params
                        .iter()
                        .map(|p| match p {
                            Param::Name(n) => format!("name:{n}"),
                            Param::Tag(t) => format!("tag:{t}"),
                        })
                        .collect::<Vec<_>>()
                        .join("|")
                },
            ),
        );

        let client = TestClient::new(app);

        let res = client.get("/?name=john%20doe&tag=hello%26world").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "name:john doe|tag:hello&world");
    }

    #[test]
    fn test_query_list_try_from_uri_with_encoded_special_characters() {
        #[allow(dead_code)]
        #[derive(Deserialize, PartialEq, Debug)]
        #[serde(rename_all = "lowercase")]
        enum Param {
            Name(String),
        }

        let uri: Uri = "http://example.com/path?name=john%20doe&name=jane%20smith"
            .parse()
            .unwrap();
        let result: QueryList<Param> = QueryList::try_from_uri(&uri).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Param::Name("john doe".to_owned()));
        assert_eq!(result[1], Param::Name("jane smith".to_owned()));
    }
}
