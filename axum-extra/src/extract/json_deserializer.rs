use axum::extract::{FromRequest, Request};
use axum_core::__composite_rejection as composite_rejection;
use axum_core::__define_rejection as define_rejection;
use axum_core::extract::rejection::BytesRejection;
use bytes::Bytes;
use http::{header, HeaderMap};
use serde::Deserialize;
use std::marker::PhantomData;

/// JSON Extractor for zero-copy deserialization.
///
/// Deserialize request bodies into some type that implements [`serde::Deserialize<'de>`][serde::Deserialize].
/// Parsing JSON is delayed until [`deserialize`](JsonDeserializer::deserialize) is called.
/// If the type implements [`serde::de::DeserializeOwned`], the [`Json`](axum::Json) extractor should
/// be preferred.
///
/// The request will be rejected (and a [`JsonDeserializerRejection`] will be returned) if:
///
/// - The request doesn't have a `Content-Type: application/json` (or similar) header.
/// - Buffering the request body fails.
///
/// Additionally, a `JsonRejection` error will be returned, when calling `deserialize` if:
///
/// - The body doesn't contain syntactically valid JSON.
/// - The body contains syntactically valid JSON, but it couldn't be deserialized into the target type.
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
/// [order-of-extractors]: axum::extract#the-order-of-extractors
///
/// See [`JsonDeserializerRejection`] for more details.
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
#[cfg_attr(docsrs, doc(cfg(feature = "json-deserializer")))]
pub struct JsonDeserializer<T> {
    bytes: Bytes,
    _marker: PhantomData<T>,
}

impl<T, S> FromRequest<S> for JsonDeserializer<T>
where
    T: Deserialize<'static>,
    S: Send + Sync,
{
    type Rejection = JsonDeserializerRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        if json_content_type(req.headers()) {
            let bytes = Bytes::from_request(req, state).await?;
            Ok(Self {
                bytes,
                _marker: PhantomData,
            })
        } else {
            Err(MissingJsonContentType.into())
        }
    }
}

impl<'de, 'a: 'de, T> JsonDeserializer<T>
where
    T: Deserialize<'de>,
{
    /// Deserialize the request body into the target type.
    /// See [`JsonDeserializer`] for more details.
    pub fn deserialize(&'a self) -> Result<T, JsonDeserializerRejection> {
        let deserializer = &mut serde_json::Deserializer::from_slice(&self.bytes);

        let value = match serde_path_to_error::deserialize(deserializer) {
            Ok(value) => value,
            Err(err) => {
                let rejection = match err.inner().classify() {
                    serde_json::error::Category::Data => JsonDataError::from_err(err).into(),
                    serde_json::error::Category::Syntax | serde_json::error::Category::Eof => {
                        JsonSyntaxError::from_err(err).into()
                    }
                    serde_json::error::Category::Io => {
                        if cfg!(debug_assertions) {
                            // we don't use `serde_json::from_reader` and instead always buffer
                            // bodies first, so we shouldn't encounter any IO errors
                            unreachable!()
                        } else {
                            JsonSyntaxError::from_err(err).into()
                        }
                    }
                };
                return Err(rejection);
            }
        };

        Ok(value)
    }
}

define_rejection! {
    #[status = UNPROCESSABLE_ENTITY]
    #[body = "Failed to deserialize the JSON body into the target type"]
    #[cfg_attr(docsrs, doc(cfg(feature = "json-deserializer")))]
    /// Rejection type for [`JsonDeserializer`].
    ///
    /// This rejection is used if the request body is syntactically valid JSON but couldn't be
    /// deserialized into the target type.
    pub struct JsonDataError(Error);
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to parse the request body as JSON"]
    #[cfg_attr(docsrs, doc(cfg(feature = "json-deserializer")))]
    /// Rejection type for [`JsonDeserializer`].
    ///
    /// This rejection is used if the request body didn't contain syntactically valid JSON.
    pub struct JsonSyntaxError(Error);
}

define_rejection! {
    #[status = UNSUPPORTED_MEDIA_TYPE]
    #[body = "Expected request with `Content-Type: application/json`"]
    #[cfg_attr(docsrs, doc(cfg(feature = "json-deserializer")))]
    /// Rejection type for [`JsonDeserializer`] used if the `Content-Type`
    /// header is missing.
    pub struct MissingJsonContentType;
}

composite_rejection! {
    /// Rejection used for [`JsonDeserializer`].
    ///
    /// Contains one variant for each way the [`JsonDeserializer`] extractor
    /// can fail.
    #[cfg_attr(docsrs, doc(cfg(feature = "json-deserializer")))]
    pub enum JsonDeserializerRejection {
        JsonDataError,
        JsonSyntaxError,
        MissingJsonContentType,
        BytesRejection,
    }
}

fn json_content_type(headers: &HeaderMap) -> bool {
    let content_type = if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        content_type
    } else {
        return false;
    };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return false;
    };

    let mime = if let Ok(mime) = content_type.parse::<mime::Mime>() {
        mime
    } else {
        return false;
    };

    let is_json_content_type = mime.type_() == "application"
        && (mime.subtype() == "json" || mime.suffix().is_some_and(|name| name == "json"));

    is_json_content_type
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
    use serde_json::{json, Value};
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
        let res = client.post("/").json(&json!({ "foo": "bar" })).await;
        let body = res.text().await;

        assert_eq!(body, "bar");
    }

    #[tokio::test]
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
        let res = client.post("/").json(&json!({ "foo": "\"bar\"" })).await;

        let body = res.text().await;

        assert_eq!(body, r#""bar""#);
    }

    #[tokio::test]
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

        let res = client.post("/").json(&json!({ "foo": "good" })).await;
        let body = res.text().await;
        assert_eq!(body, "good");

        let res = client.post("/").json(&json!({ "foo": "\"bad\"" })).await;
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body_text = res.text().await;
        assert_eq!(
            body_text,
            "Failed to deserialize the JSON body into the target type: foo: invalid type: string \"\\\"bad\\\"\", expected a borrowed string at line 1 column 16"
        );
    }

    #[tokio::test]
    async fn consume_body_to_json_requires_json_content_type() {
        #[derive(Debug, Deserialize)]
        struct Input<'a> {
            #[allow(dead_code)]
            foo: Cow<'a, str>,
        }

        async fn handler(_deserializer: JsonDeserializer<Input<'_>>) -> Response {
            panic!("This handler should not be called")
        }

        let app = Router::new().route("/", post(handler));

        let client = TestClient::new(app);
        let res = client.post("/").body(r#"{ "foo": "bar" }"#).await;

        let status = res.status();

        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn json_content_types() {
        async fn valid_json_content_type(content_type: &str) -> bool {
            println!("testing {content_type:?}");

            async fn handler(_deserializer: JsonDeserializer<Value>) -> Response {
                StatusCode::OK.into_response()
            }

            let app = Router::new().route("/", post(handler));

            let res = TestClient::new(app)
                .post("/")
                .header("content-type", content_type)
                .body("{}")
                .await;

            res.status() == StatusCode::OK
        }

        assert!(valid_json_content_type("application/json").await);
        assert!(valid_json_content_type("application/json; charset=utf-8").await);
        assert!(valid_json_content_type("application/json;charset=utf-8").await);
        assert!(valid_json_content_type("application/cloudevents+json").await);
        assert!(!valid_json_content_type("text/json").await);
    }

    #[tokio::test]
    async fn invalid_json_syntax() {
        async fn handler(deserializer: JsonDeserializer<Value>) -> Response {
            match deserializer.deserialize() {
                Ok(_) => panic!("Should have matched `Err`"),
                Err(e) => e.into_response(),
            }
        }

        let app = Router::new().route("/", post(handler));

        let client = TestClient::new(app);
        let res = client
            .post("/")
            .body("{")
            .header("content-type", "application/json")
            .await;

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[derive(Deserialize)]
    struct Foo {
        #[allow(dead_code)]
        a: i32,
        #[allow(dead_code)]
        b: Vec<Bar>,
    }

    #[derive(Deserialize)]
    struct Bar {
        #[allow(dead_code)]
        x: i32,
        #[allow(dead_code)]
        y: i32,
    }

    #[tokio::test]
    async fn invalid_json_data() {
        async fn handler(deserializer: JsonDeserializer<Foo>) -> Response {
            match deserializer.deserialize() {
                Ok(_) => panic!("Should have matched `Err`"),
                Err(e) => e.into_response(),
            }
        }

        let app = Router::new().route("/", post(handler));

        let client = TestClient::new(app);
        let res = client
            .post("/")
            .body("{\"a\": 1, \"b\": [{\"x\": 2}]}")
            .header("content-type", "application/json")
            .await;

        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body_text = res.text().await;
        assert_eq!(
            body_text,
            "Failed to deserialize the JSON body into the target type: b[0]: missing field `y` at line 1 column 23"
        );
    }
}
