use crate::{
    body::{self, Bytes, Full, HttpBody},
    extract::{rejection::*, FromRequest, RequestParts},
    response::{IntoResponse, Response},
    BoxError,
};
use async_trait::async_trait;
use http::{
    header::{self, HeaderValue},
    StatusCode,
};
use serde::{de::DeserializeOwned, Serialize};
use std::ops::{Deref, DerefMut};

/// MessagePack Extractor / Response.
///
/// When used as an extractor, it can deserialize request bodies into some type that
/// implements [`serde::Deserialize`]. If the request body cannot be parsed, or value of the 
/// `Content-Type` header does not match any of the `application/msgpack`, `application/x-msgpack`
/// or `application/*+msgpack` it will reject the request and return a `400 Bad Request` response.
///
/// # Extractor example
///
/// ```rust,no_run
/// use axum::{
///     extract::MessagePack,
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
/// async fn create_user(MessagePack(payload): MessagePack<CreateUser>) {
///     // payload is a `CreateUser`
/// }
///
/// let app = Router::new().route("/users", post(create_user));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// When used as a response, it can serialize any type that implements [`serde::Serialize`] to
/// `MessagePack`, and will automatically set `Content-Type: application/msgpack` header.
///
/// # Response example
///
/// ```
/// use axum::{
///     extract::{Path, MessagePack},
///     routing::get,
///     Router,
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
/// async fn get_user(Path(user_id) : Path<Uuid>) -> MessagePack<User> {
///     let user = find_user(user_id).await;
///     MessagePack(user)
/// }
///
/// async fn find_user(user_id: Uuid) -> User {
///     // ...
///     # unimplemented!()
/// }
///
/// let app = Router::new().route("/users/:id", get(get_user));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(docsrs, doc(cfg(feature = "messagepack")))]
pub struct MessagePack<T>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for MessagePack<T> 
where 
    T: DeserializeOwned,
    B: HttpBody + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = MessagePackRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if message_pack_content_type(req)? {
            let bytes = Bytes::from_request(req).await?;
            
            let value = rmp_serde::from_read_ref(&bytes)
                .map_err(InvalidMessagePackBody::from_err)?;

            Ok(MessagePack(value))
        } else {
            Err(MissingMessagePackContentType.into())
        }
    }
}

fn message_pack_content_type<B>(req: &RequestParts<B>) -> Result<bool, HeadersAlreadyExtracted> {
    let content_type = if let Some(content_type) = req 
        .headers()
        .ok_or_else(HeadersAlreadyExtracted::default)?
        .get(header::CONTENT_TYPE) {
            content_type
        } else {
            return Ok(false)
        };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return Ok(false)
    };

    let mime = if let Ok(mime) = content_type.parse::<mime::Mime>() {
        mime
    } else {
        return Ok(false)
    };

    let is_rmp = mime.type_() == "application" && (
        ["msgpack", "x-msgpack"].iter()
            .any(|subtype| *subtype == mime.subtype()) ||
        mime.suffix().map_or(false, |suffix| suffix == "msgpack"));

    Ok(is_rmp)
}

impl<T> Deref for MessagePack<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for MessagePack<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> From<T> for MessagePack<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> IntoResponse for MessagePack<T> 
    where 
        T: Serialize,
{
    fn into_response(self) -> Response {
        let bytes = match rmp_serde::to_vec(&self.0) {
            Ok(bytes) => bytes,
            Err(err) => {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()))
                    .body(body::boxed(Full::from(err.to_string())))
                    .unwrap();
            },
        };

        let mut res = Response::new(body::boxed(Full::from(bytes)));
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(mime::MSGPACK.as_ref()));

        res
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::{routing::post, test_helpers::*, Router};
    use serde::Deserialize;

    #[derive(Debug, Serialize, Deserialize)]
    struct Input {
        foo: String,
    }

    #[tokio::test]
    async fn deserialize_body() {
        let app = Router::new().route("/", post(|input: MessagePack<Input>| async { input.0.foo }));
        let client = TestClient::new(app);
        let body = rmp_serde::to_vec(&Input {foo: "bar".into()}).expect("Failed to serialize to MessagePack");
        let res = client.post("/").body(body).header("Content-Type", "application/msgpack").send().await;

        let body = res.text().await;

        assert_eq!(body, "bar");
    }

    #[tokio::test]
    async fn consume_body_to_messagepack_requires_messagepack_content_type() {

        let app = Router::new().route("/", post(|input: MessagePack<Input>| async { input.0.foo }));

        let client = TestClient::new(app);
        let body = rmp_serde::to_vec(&Input {foo: "bar".into()}).expect("Failed to serialize to MessagePack");
        let res = client.post("/").body(body).send().await;

        let status = res.status();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn json_content_types() {
        async fn valid_json_content_type(content_type: &str) -> bool {
            println!("testing {:?}", content_type);
            let body = rmp_serde::to_vec(&Input {foo: "bar".into()}).expect("Failed to serialize to MessagePack");
            let app = Router::new().route("/", post(|MessagePack(_): MessagePack<Input>| async {}));

            let res = TestClient::new(app)
                .post("/")
                .header("content-type", content_type)
                .body(body)
                .send()
                .await;

            res.status() == StatusCode::OK
        }

        assert!(valid_json_content_type("application/msgpack").await);
        assert!(valid_json_content_type("application/msgpack; charset=utf-8").await);
        assert!(valid_json_content_type("application/msgpack;charset=utf-8").await);
        assert!(valid_json_content_type("application/cloudevents+msgpack").await);
        assert!(!valid_json_content_type("text/json").await);
    }
}
