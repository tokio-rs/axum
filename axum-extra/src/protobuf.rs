//! Protocol Buffer extractor and response.

use axum::{
    extract::{rejection::BytesRejection, FromRequest, Request},
    response::{IntoResponse, Response},
};
use axum_core::__composite_rejection as composite_rejection;
use axum_core::__define_rejection as define_rejection;
use bytes::{Bytes, BytesMut};
use http::StatusCode;
use prost::Message;

/// A Protocol Buffer message extractor and response.
///
/// This can be used both as an extractor and as a response.
///
/// # As extractor
///
/// When used as an extractor, it can decode request bodies into some type that
/// implements [`prost::Message`]. The request will be rejected (and a [`ProtobufRejection`] will
/// be returned) if:
///
/// - The body couldn't be decoded into the target Protocol Buffer message type.
/// - Buffering the request body fails.
///
/// See [`ProtobufRejection`] for more details.
///
/// The extractor does not expect a `Content-Type` header to be present in the request.
///
/// # Extractor example
///
/// ```rust,no_run
/// use axum::{routing::post, Router};
/// use axum_extra::protobuf::Protobuf;
///
/// #[derive(prost::Message)]
/// struct CreateUser {
///     #[prost(string, tag="1")]
///     email: String,
///     #[prost(string, tag="2")]
///     password: String,
/// }
///
/// async fn create_user(Protobuf(payload): Protobuf<CreateUser>) {
///     // payload is `CreateUser`
/// }
///
/// let app = Router::new().route("/users", post(create_user));
/// # let _: Router = app;
/// ```
///
/// # As response
///
/// When used as a response, it can encode any type that implements [`prost::Message`] to
/// a newly allocated buffer.
///
/// If no `Content-Type` header is set, the `Content-Type: application/octet-stream` header
/// will be used automatically.
///
/// # Response example
///
/// ```
/// use axum::{
///     extract::Path,
///     routing::get,
///     Router,
/// };
/// use axum_extra::protobuf::Protobuf;
///
/// #[derive(prost::Message)]
/// struct User {
///     #[prost(string, tag="1")]
///     username: String,
/// }
///
/// async fn get_user(Path(user_id) : Path<String>) -> Protobuf<User> {
///     let user = find_user(user_id).await;
///     Protobuf(user)
/// }
///
/// async fn find_user(user_id: String) -> User {
///     // ...
///     # unimplemented!()
/// }
///
/// let app = Router::new().route("/users/{id}", get(get_user));
/// # let _: Router = app;
/// ```
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(docsrs, doc(cfg(feature = "protobuf")))]
#[must_use]
pub struct Protobuf<T>(pub T);

impl<T, S> FromRequest<S> for Protobuf<T>
where
    T: Message + Default,
    S: Send + Sync,
{
    type Rejection = ProtobufRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let mut bytes = Bytes::from_request(req, state).await?;

        match T::decode(&mut bytes) {
            Ok(value) => Ok(Protobuf(value)),
            Err(err) => Err(ProtobufDecodeError::from_err(err).into()),
        }
    }
}

axum_core::__impl_deref!(Protobuf);

impl<T> From<T> for Protobuf<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> IntoResponse for Protobuf<T>
where
    T: Message + Default,
{
    fn into_response(self) -> Response {
        let mut buf = BytesMut::with_capacity(128);
        match &self.0.encode(&mut buf) {
            Ok(()) => buf.into_response(),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        }
    }
}

define_rejection! {
    #[status = UNPROCESSABLE_ENTITY]
    #[body = "Failed to decode the body"]
    /// Rejection type for [`Protobuf`].
    ///
    /// This rejection is used if the request body couldn't be decoded into the target type.
    pub struct ProtobufDecodeError(Error);
}

composite_rejection! {
    /// Rejection used for [`Protobuf`].
    ///
    /// Contains one variant for each way the [`Protobuf`] extractor
    /// can fail.
    pub enum ProtobufRejection {
        ProtobufDecodeError,
        BytesRejection,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{routing::post, Router};

    #[tokio::test]
    async fn decode_body() {
        #[derive(prost::Message)]
        struct Input {
            #[prost(string, tag = "1")]
            foo: String,
        }

        let app = Router::new().route(
            "/",
            post(|input: Protobuf<Input>| async move { input.foo.to_owned() }),
        );

        let input = Input {
            foo: "bar".to_owned(),
        };

        let client = TestClient::new(app);
        let res = client.post("/").body(input.encode_to_vec()).await;

        let body = res.text().await;

        assert_eq!(body, "bar");
    }

    #[tokio::test]
    async fn prost_decode_error() {
        #[derive(prost::Message)]
        struct Input {
            #[prost(string, tag = "1")]
            foo: String,
        }

        #[derive(prost::Message)]
        struct Expected {
            #[prost(int32, tag = "1")]
            test: i32,
        }

        let app = Router::new().route("/", post(|_: Protobuf<Expected>| async {}));

        let input = Input {
            foo: "bar".to_owned(),
        };

        let client = TestClient::new(app);
        let res = client.post("/").body(input.encode_to_vec()).await;

        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn encode_body() {
        #[derive(prost::Message)]
        struct Input {
            #[prost(string, tag = "1")]
            foo: String,
        }

        #[derive(prost::Message)]
        struct Output {
            #[prost(string, tag = "1")]
            result: String,
        }

        #[axum::debug_handler]
        async fn handler(input: Protobuf<Input>) -> Protobuf<Output> {
            let output = Output {
                result: input.foo.to_owned(),
            };

            Protobuf(output)
        }

        let app = Router::new().route("/", post(handler));

        let input = Input {
            foo: "bar".to_owned(),
        };

        let client = TestClient::new(app);
        let res = client.post("/").body(input.encode_to_vec()).await;

        assert_eq!(
            res.headers()["content-type"],
            mime::APPLICATION_OCTET_STREAM.as_ref()
        );

        let body = res.bytes().await;

        let output = Output::decode(body).unwrap();

        assert_eq!(output.result, "bar");
    }
}
