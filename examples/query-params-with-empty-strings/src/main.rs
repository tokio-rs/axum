//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-query-params-with-empty-strings
//! ```

use axum::{extract::Query, routing::get, Router};
use serde::{de, Deserialize, Deserializer};
use std::{fmt, str::FromStr};

#[tokio::main]
async fn main() {
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app().into_make_service())
        .await
        .unwrap();
}

fn app() -> Router {
    Router::new().route("/", get(handler))
}

async fn handler(Query(params): Query<Params>) -> String {
    format!("{:?}", params)
}

/// See the tests below for which combinations of `foo` and `bar` result in
/// which deserializations.
///
/// This example only shows one possible way to do this. [`serde_with`] provides
/// another way. Use which ever method works best for you.
///
/// [`serde_with`]: https://docs.rs/serde_with/1.11.0/serde_with/rust/string_empty_as_none/index.html
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Params {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    foo: Option<i32>,
    bar: Option<String>,
}

/// Serde deserialization decorator to map empty Strings to None,
fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s).map_err(de::Error::custom).map(Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_something() {
        assert_eq!(
            send_request_get_body("foo=1&bar=bar").await,
            r#"Params { foo: Some(1), bar: Some("bar") }"#,
        );

        assert_eq!(
            send_request_get_body("foo=&bar=bar").await,
            r#"Params { foo: None, bar: Some("bar") }"#,
        );

        assert_eq!(
            send_request_get_body("foo=&bar=").await,
            r#"Params { foo: None, bar: Some("") }"#,
        );

        assert_eq!(
            send_request_get_body("foo=1").await,
            r#"Params { foo: Some(1), bar: None }"#,
        );

        assert_eq!(
            send_request_get_body("bar=bar").await,
            r#"Params { foo: None, bar: Some("bar") }"#,
        );

        assert_eq!(
            send_request_get_body("foo=").await,
            r#"Params { foo: None, bar: None }"#,
        );

        assert_eq!(
            send_request_get_body("bar=").await,
            r#"Params { foo: None, bar: Some("") }"#,
        );

        assert_eq!(
            send_request_get_body("").await,
            r#"Params { foo: None, bar: None }"#,
        );
    }

    async fn send_request_get_body(query: &str) -> String {
        let body = app()
            .oneshot(
                Request::builder()
                    .uri(format!("/?{}", query))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .into_body();
        let bytes = hyper::body::to_bytes(body).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }
}
