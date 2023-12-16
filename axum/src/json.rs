use crate::extract::Request;
use crate::extract::{rejection::*, FromRequest};
use async_trait::async_trait;
use axum_core::response::{IntoResponse, Response};
use bytes::{BufMut, Bytes, BytesMut};
use http::{
    header::{self, HeaderMap, HeaderValue},
    StatusCode,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::marker::PhantomData;

/// JSON Extractor / Response.
///
/// When used as an extractor, it can deserialize request bodies into some type that
/// implements [`serde::de::DeserializeOwned`]. The request will be rejected (and a [`JsonRejection`] will
/// be returned) if:
///
/// - The request doesn't have a `Content-Type: application/json` (or similar) header.
/// - The body doesn't contain syntactically valid JSON.
/// - The body contains syntactically valid JSON, but it couldn't be deserialized into the target
/// type.
/// - Buffering the request body fails.
///
/// ⚠️ Since parsing JSON requires consuming the request body, the `Json` extractor must be
/// *last* if there are multiple extractors in a handler.
/// See ["the order of extractors"][order-of-extractors]
///
/// [order-of-extractors]: crate::extract#the-order-of-extractors
///
/// See [`JsonRejection`] for more details.
///
/// # Extractor example
///
/// ```rust,no_run
/// use axum::{
///     extract,
///     routing::post,
///     Router,
/// };
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct CreateUser {
///     email: String,
///     password: String,
/// }
///
/// async fn create_user(extract::Json(payload): extract::Json<CreateUser>) {
///     // payload is a `CreateUser`
/// }
///
/// let app = Router::new().route("/users", post(create_user));
/// # let _: Router = app;
/// ```
///
/// When used as a response, it can serialize any type that implements [`serde::Serialize`] to
/// `JSON`, and will automatically set `Content-Type: application/json` header.
///
/// # Response example
///
/// ```
/// use axum::{
///     extract::Path,
///     routing::get,
///     Router,
///     Json,
/// };
/// use serde::Serialize;
/// use uuid::Uuid;
///
/// #[derive(Serialize)]
/// struct User {
///     id: Uuid,
///     username: String,
/// }
///
/// async fn get_user(Path(user_id) : Path<Uuid>) -> Json<User> {
///     let user = find_user(user_id).await;
///     Json(user)
/// }
///
/// async fn find_user(user_id: Uuid) -> User {
///     // ...
///     # unimplemented!()
/// }
///
/// let app = Router::new().route("/users/:id", get(get_user));
/// # let _: Router = app;
/// ```
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
#[must_use]
pub struct Json<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = JsonRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        if json_content_type(req.headers()) {
            let bytes = Bytes::from_request(req, state).await?;
            Self::from_bytes(&bytes)
        } else {
            Err(MissingJsonContentType.into())
        }
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
        && (mime.subtype() == "json" || mime.suffix().map_or(false, |name| name == "json"));

    is_json_content_type
}

fn json_from_bytes<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, JsonRejection> {
    let deserializer = &mut serde_json::Deserializer::from_slice(bytes);

    match serde_path_to_error::deserialize(deserializer) {
        Ok(value) => Ok(value),
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
            Err(rejection)
        }
    }
}

axum_core::__impl_deref!(Json);

impl<T> From<T> for Json<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> Json<T>
where
    T: DeserializeOwned,
{
    /// Construct a `Json<T>` from a byte slice. Most users should prefer to use the `FromRequest` impl
    /// but special cases may require first extracting a `Request` into `Bytes` then optionally
    /// constructing a `Json<T>`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, JsonRejection> {
        let value = json_from_bytes(bytes)?;
        Ok(Json(value))
    }
}

impl<T> IntoResponse for Json<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        // Use a small initial capacity of 128 bytes like serde_json::to_vec
        // https://docs.rs/serde_json/1.0.82/src/serde_json/ser.rs.html#2189
        let mut buf = BytesMut::with_capacity(128).writer();
        match serde_json::to_writer(&mut buf, &self.0) {
            Ok(()) => (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
                )],
                buf.into_inner().freeze(),
            )
                .into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
                )],
                err.to_string(),
            )
                .into_response(),
        }
    }
}

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
///     extract,
///     routing::post,
///     Router,
///     response::{IntoResponse, Response}
/// };
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
/// async fn upload(deserializer: extract::JsonDeserializer<Data<'_>>) -> Response {
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
    pub fn deserialize(&'a self) -> Result<T, JsonRejection> {
        let value = json_from_bytes(&self.bytes)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{routing::post, test_helpers::*, Router};
    use serde::Deserialize;
    use serde_json::{json, Value};
    use std::borrow::Cow;

    #[crate::test]
    async fn deserialize_body() {
        #[derive(Debug, Deserialize)]
        struct Input {
            foo: String,
        }

        let app = Router::new().route("/", post(|input: Json<Input>| async { input.0.foo }));

        let client = TestClient::new(app);
        let res = client.post("/").json(&json!({ "foo": "bar" })).send().await;
        let body = res.text().await;

        assert_eq!(body, "bar");
    }

    #[crate::test]
    async fn deserializer_deserialize_body() {
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
    async fn deserializer_deserialize_body_escaped_to_cow() {
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
    async fn deserializer_deserialize_body_escaped_to_str() {
        #[derive(Debug, Deserialize)]
        struct Input<'a> {
            // Explicit `#[serde(borrow)]` attribute is not required for `&str` or &[u8].
            // See: https://serde.rs/lifetimes.html#borrowing-data-in-a-derived-impl
            foo: &'a str,
        }

        async fn route_fn(deserializer: JsonDeserializer<Input<'_>>) -> Response {
            match deserializer.deserialize() {
                Ok(Input { foo }) => foo.to_owned().into_response(),
                Err(e) => e.into_response(),
            }
        }

        let app = Router::new().route("/", post(route_fn));

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

    #[crate::test]
    async fn consume_body_to_json_requires_json_content_type() {
        #[derive(Debug, Deserialize)]
        struct Input {
            foo: String,
        }

        let app = Router::new().route("/", post(|input: Json<Input>| async { input.0.foo }));

        let client = TestClient::new(app);
        let res = client.post("/").body(r#"{ "foo": "bar" }"#).send().await;

        let status = res.status();

        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[crate::test]
    async fn json_content_types() {
        async fn valid_json_content_type(content_type: &str) -> bool {
            println!("testing {content_type:?}");

            let app = Router::new().route("/", post(|Json(_): Json<Value>| async {}));

            let res = TestClient::new(app)
                .post("/")
                .header("content-type", content_type)
                .body("{}")
                .send()
                .await;

            res.status() == StatusCode::OK
        }

        assert!(valid_json_content_type("application/json").await);
        assert!(valid_json_content_type("application/json; charset=utf-8").await);
        assert!(valid_json_content_type("application/json;charset=utf-8").await);
        assert!(valid_json_content_type("application/cloudevents+json").await);
        assert!(!valid_json_content_type("text/json").await);
    }

    #[crate::test]
    async fn invalid_json_syntax() {
        let app = Router::new().route("/", post(|_: Json<serde_json::Value>| async {}));

        let client = TestClient::new(app);
        let res = client
            .post("/")
            .body("{")
            .header("content-type", "application/json")
            .send()
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

    #[crate::test]
    async fn invalid_json_data() {
        let app = Router::new().route("/", post(|_: Json<Foo>| async {}));

        let client = TestClient::new(app);
        let res = client
            .post("/")
            .body("{\"a\": 1, \"b\": [{\"x\": 2}]}")
            .header("content-type", "application/json")
            .send()
            .await;

        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body_text = res.text().await;
        assert_eq!(
            body_text,
            "Failed to deserialize the JSON body into the target type: b[0]: missing field `y` at line 1 column 23"
        );
    }
}
