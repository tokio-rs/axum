//! Protocol Buffer extractor and response.

use axum::{
    async_trait,
    body::{Bytes, HttpBody},
    extract::{rejection::BytesRejection, FromRequest},
    response::{IntoResponse, Response},
    BoxError,
};
use bytes::BytesMut;
use http::{Request, StatusCode};
use prost::Message;
use std::ops::{Deref, DerefMut};

/// A Protocol Buffer message extractor and response.
///
/// This can be used both as an extractor and as a response.
///
/// # As extractor
///
/// When used as an extractor, it can decode request bodies into some type that
/// implements [`prost::Message`]. The request will be rejected (and a [`ProtoBufRejection`] will
/// be returned) if:
///
/// - The body couldn't be decoded into the target Protocol Buffer message type.
/// - Buffering the request body fails.
///
/// See [`ProtoBufRejection`] for more details.
///
/// The extractor does not expect a `Content-Type` header to be present in the request.
///
/// # Extractor example
///
/// ```rust,no_run
/// use axum::{routing::post, Router};
/// use axum_extra::protobuf::ProtoBuf;
///
/// #[derive(prost::Message)]
/// struct CreateUser {
///     #[prost(string, tag="1")]
///     email: String,
///     #[prost(string, tag="2")]
///     password: String,
/// }
///
/// async fn create_user(ProtoBuf(payload): ProtoBuf<CreateUser>) {
///     // payload is `CreateUser`
/// }
///
/// let app = Router::new().route("/users", post(create_user));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
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
/// use axum_extra::protobuf::ProtoBuf;
///
/// #[derive(prost::Message)]
/// struct User {
///     #[prost(string, tag="1")]
///     username: String,
/// }
///
/// async fn get_user(Path(user_id) : Path<String>) -> ProtoBuf<User> {
///     let user = find_user(user_id).await;
///     ProtoBuf(user)
/// }
///
/// async fn find_user(user_id: String) -> User {
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
#[cfg_attr(docsrs, doc(cfg(feature = "protobuf")))]
pub struct ProtoBuf<T>(pub T);

#[async_trait]
impl<T, S, B> FromRequest<S, B> for ProtoBuf<T>
where
    T: Message + Default,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = ProtoBufRejection;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let mut bytes = Bytes::from_request(req, state).await?;

        match T::decode(&mut bytes) {
            Ok(value) => Ok(ProtoBuf(value)),
            Err(err) => Err(ProtoBufDecodeError::from_err(err).into()),
        }
    }
}

impl<T> Deref for ProtoBuf<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for ProtoBuf<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> From<T> for ProtoBuf<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> IntoResponse for ProtoBuf<T>
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

/// Rejection type for [`ProtoBuf`].
///
/// This rejection is used if the request body couldn't be decoded into the target type.
#[derive(Debug)]
pub struct ProtoBufDecodeError(pub(crate) axum::Error);

impl ProtoBufDecodeError {
    pub(crate) fn from_err<E>(err: E) -> Self
    where
        E: Into<axum::BoxError>,
    {
        Self(axum::Error::new(err))
    }
}

impl std::fmt::Display for ProtoBufDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to decode the body: {:?}", self.0)
    }
}

impl std::error::Error for ProtoBufDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl IntoResponse for ProtoBufDecodeError {
    fn into_response(self) -> Response {
        StatusCode::UNPROCESSABLE_ENTITY.into_response()
    }
}

/// Rejection used for [`ProtoBuf`].
///
/// Contains one variant for each way the [`ProtoBuf`] extractor
/// can fail.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtoBufRejection {
    #[allow(missing_docs)]
    ProtoBufDecodeError(ProtoBufDecodeError),
    #[allow(missing_docs)]
    BytesRejection(BytesRejection),
}

impl From<ProtoBufDecodeError> for ProtoBufRejection {
    fn from(inner: ProtoBufDecodeError) -> Self {
        Self::ProtoBufDecodeError(inner)
    }
}

impl From<BytesRejection> for ProtoBufRejection {
    fn from(inner: BytesRejection) -> Self {
        Self::BytesRejection(inner)
    }
}

impl IntoResponse for ProtoBufRejection {
    fn into_response(self) -> Response {
        match self {
            Self::ProtoBufDecodeError(inner) => inner.into_response(),
            Self::BytesRejection(inner) => inner.into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{routing::post, Router};
    use http::StatusCode;

    #[tokio::test]
    async fn decode_body() {
        #[derive(prost::Message)]
        struct Input {
            #[prost(string, tag = "1")]
            foo: String,
        }

        let app = Router::new().route(
            "/",
            post(|input: ProtoBuf<Input>| async move { input.foo.to_owned() }),
        );

        let input = Input {
            foo: "bar".to_owned(),
        };

        let client = TestClient::new(app);
        let res = client.post("/").body(input.encode_to_vec()).send().await;

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

        let app = Router::new().route("/", post(|_: ProtoBuf<Expected>| async {}));

        let input = Input {
            foo: "bar".to_owned(),
        };

        let client = TestClient::new(app);
        let res = client.post("/").body(input.encode_to_vec()).send().await;

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

        let app = Router::new().route(
            "/",
            post(|input: ProtoBuf<Input>| async move {
                let output = Output {
                    result: input.foo.to_owned(),
                };

                ProtoBuf(output)
            }),
        );

        let input = Input {
            foo: "bar".to_owned(),
        };

        let client = TestClient::new(app);
        let res = client.post("/").body(input.encode_to_vec()).send().await;

        assert_eq!(
            res.headers()["content-type"],
            mime::APPLICATION_OCTET_STREAM.as_ref()
        );

        let body = res.bytes().await;

        let output = Output::decode(body).unwrap();

        assert_eq!(output.result, "bar");
    }
}
