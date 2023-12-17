use axum::async_trait;
use axum::extract::{json_helpers::*, rejection::JsonRejection, FromRequest, Request};
use bytes::Bytes;
use serde::Deserialize;
use std::marker::PhantomData;

/// JSON Extractor for zero-copy deserialization.
///
/// Deserialize request bodies into some type that implements [`serde::Deserialize<'de>`].
/// Parsing JSON is delayed until [`deserialize`](JsonDeserializer::deserialize) is called.
/// If the type implements [`serde::de::DeserializeOwned`], the [`Json`] extractor should
/// be preferred.
///
/// The request will be rejected (and a [`JsonRejection`] will be returned) if:
///
/// - The request doesn't have a `Content-Type: application/json` (or similar) header.
/// - Buffering the request body fails.
///
/// Additionally, a `JsonRejection` error will be returned, when calling `deserialize` if:
///
/// - The body doesn't contain syntactically valid JSON.
/// - The body contains syntactically valid JSON, but it couldn't be deserialized into the target
/// type.
/// - Attempting to deserialize escaped JSON into a type that must be borrowed (e.g. `&'a str`).
///
/// ⚠️ `serde` will implicitly try to borrow for `&str` and `&[u8]` types, but will error if the
/// input contains escaped characters. Use `Cow<'a, str>` or `Cow<'a, [u8]>`, with the
/// `#[serde(borrow)]` attribute, to allow serde to fall back to an owned type when encountering
/// escaped characters.
///
/// ⚠️ Since parsing JSON requires consuming the request body, the `Json` extractor must be
/// *last* if there are multiple extractors in a handler.
/// See ["the order of extractors"][order-of-extractors]
///
/// # Example
///
/// ```rust,no_run
/// use axum::{
///     routing::post,
///     Router,
///     response::{IntoResponse, Response}
/// };
/// use axum_extra::extract::JsonDeserializer;
/// use serde::Deserialize;
/// use std::borrow::Cow;
/// use http::StatusCode;
///
/// #[derive(Deserialize)]
/// struct Data<'a> {
///     #[serde(borrow)]
///     borrow_text: Cow<'a, str>,
///     #[serde(borrow)]
///     borrow_bytes: Cow<'a, [u8]>,
///     borrow_dangerous: &'a str,
///     not_borrowed: String,
/// }
///
/// async fn upload(deserializer: JsonDeserializer<Data<'_>>) -> Response {
///     let data = match deserializer.deserialize() {
///         Ok(data) => data,
///         Err(e) => return e.into_response(),
///     };
///
///     // payload is a `Data` with borrowed data from `deserializer`,
///     // which owns the request body (`Bytes`).
///
///     StatusCode::OK.into_response()
/// }
///
/// let app = Router::new().route("/upload", post(upload));
/// # let _: Router = app;
/// ```
#[derive(Debug, Clone, Default)]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
pub struct JsonDeserializer<T> {
    bytes: Bytes,
    _marker: PhantomData<T>,
}

#[async_trait]
impl<T, S> FromRequest<S> for JsonDeserializer<T>
where
    T: Deserialize<'static>,
    S: Send + Sync,
{
    type Rejection = JsonRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        if json_content_type(req.headers()) {
            let bytes = Bytes::from_request(req, state).await?;
            Ok(Self {
                bytes,
                _marker: PhantomData,
            })
        } else {
            Err(missing_json_content_type().into())
        }
    }
}

impl<'de, 'a: 'de, T> JsonDeserializer<T>
where
    T: Deserialize<'de>,
{
    /// Deserialize the request body into the target type.
    /// See [`JsonDeserializer`] for more details.
    pub fn deserialize(&'a self) -> Result<T, JsonRejection> {
        let value = json_from_bytes(&self.bytes)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{
        response::{IntoResponse, Response},
        routing::post,
        Router,
    };
    use http::StatusCode;
    use serde::Deserialize;
    use serde_json::json;
    use std::borrow::Cow;

    #[tokio::test]
    async fn deserialize_body() {
        #[derive(Debug, Deserialize)]
        struct Input<'a> {
            #[serde(borrow)]
            foo: Cow<'a, str>,
        }

        async fn handler(deserializer: JsonDeserializer<Input<'_>>) -> Response {
            match deserializer.deserialize() {
                Ok(input) => {
                    assert!(matches!(input.foo, Cow::Borrowed(_)));
                    input.foo.into_owned().into_response()
                }
                Err(e) => e.into_response(),
            }
        }

        let app = Router::new().route("/", post(handler));

        let client = TestClient::new(app);
        let res = client.post("/").json(&json!({ "foo": "bar" })).send().await;
        let body = res.text().await;

        assert_eq!(body, "bar");
    }

    #[crate::test]
    async fn deserialize_body_escaped_to_cow() {
        #[derive(Debug, Deserialize)]
        struct Input<'a> {
            #[serde(borrow)]
            foo: Cow<'a, str>,
        }

        async fn handler(deserializer: JsonDeserializer<Input<'_>>) -> Response {
            match deserializer.deserialize() {
                Ok(Input { foo }) => {
                    let Cow::Owned(foo) = foo else {
                        panic!("Deserializer is expected to fallback to Cow::Owned when encountering escaped characters")
                    };

                    foo.into_response()
                }
                Err(e) => e.into_response(),
            }
        }

        let app = Router::new().route("/", post(handler));

        let client = TestClient::new(app);

        // The escaped characters prevent serde_json from borrowing.
        let res = client
            .post("/")
            .json(&json!({ "foo": "\"bar\"" }))
            .send()
            .await;

        let body = res.text().await;

        assert_eq!(body, r#""bar""#);
    }

    #[crate::test]
    async fn deserialize_body_escaped_to_str() {
        #[derive(Debug, Deserialize)]
        struct Input<'a> {
            // Explicit `#[serde(borrow)]` attribute is not required for `&str` or &[u8].
            // See: https://serde.rs/lifetimes.html#borrowing-data-in-a-derived-impl
            foo: &'a str,
        }

        async fn handler(deserializer: JsonDeserializer<Input<'_>>) -> Response {
            match deserializer.deserialize() {
                Ok(Input { foo }) => foo.to_owned().into_response(),
                Err(e) => e.into_response(),
            }
        }

        let app = Router::new().route("/", post(handler));

        let client = TestClient::new(app);

        let res = client
            .post("/")
            .json(&json!({ "foo": "good" }))
            .send()
            .await;
        let body = res.text().await;
        assert_eq!(body, "good");

        let res = client
            .post("/")
            .json(&json!({ "foo": "\"bad\"" }))
            .send()
            .await;
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body_text = res.text().await;
        assert_eq!(
            body_text,
            "Failed to deserialize the JSON body into the target type: foo: invalid type: string \"\\\"bad\\\"\", expected a borrowed string at line 1 column 16"
        );
    }
}
