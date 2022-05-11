//! Extractor that will get captures from the URL and parse them using
//! [`serde`].

mod de;

use crate::{
    extract::{rejection::*, FromRequest, RequestParts},
    routing::url_params::UrlParams,
};
use async_trait::async_trait;
use axum_core::response::{IntoResponse, Response};
use http::StatusCode;
use serde::de::DeserializeOwned;
use std::{
    fmt,
    ops::{Deref, DerefMut},
};

/// Extractor that will get captures from the URL and parse them using
/// [`serde`].
///
/// Any percent encoded parameters will be automatically decoded. The decoded
/// parameters must be valid UTF-8, otherwise `Path` will fail and return a `400
/// Bad Request` response.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{
///     extract::Path,
///     routing::get,
///     Router,
/// };
/// use uuid::Uuid;
///
/// async fn users_teams_show(
///     Path((user_id, team_id)): Path<(Uuid, Uuid)>,
/// ) {
///     // ...
/// }
///
/// let app = Router::new().route("/users/:user_id/team/:team_id", get(users_teams_show));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If the path contains only one parameter, then you can omit the tuple.
///
/// ```rust,no_run
/// use axum::{
///     extract::Path,
///     routing::get,
///     Router,
/// };
/// use uuid::Uuid;
///
/// async fn user_info(Path(user_id): Path<Uuid>) {
///     // ...
/// }
///
/// let app = Router::new().route("/users/:user_id", get(user_info));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Path segments also can be deserialized into any type that implements
/// [`serde::Deserialize`]. This includes tuples and structs:
///
/// ```rust,no_run
/// use axum::{
///     extract::Path,
///     routing::get,
///     Router,
/// };
/// use serde::Deserialize;
/// use uuid::Uuid;
///
/// // Path segment labels will be matched with struct field names
/// #[derive(Deserialize)]
/// struct Params {
///     user_id: Uuid,
///     team_id: Uuid,
/// }
///
/// async fn users_teams_show(
///     Path(Params { user_id, team_id }): Path<Params>,
/// ) {
///     // ...
/// }
///
/// // When using tuples the path segments will be matched by their position in the route
/// async fn users_teams_create(
///     Path((user_id, team_id)): Path<(String, String)>,
/// ) {
///     // ...
/// }
///
/// let app = Router::new().route(
///     "/users/:user_id/team/:team_id",
///     get(users_teams_show).post(users_teams_create),
/// );
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If you wish to capture all path parameters you can use `HashMap` or `Vec`:
///
/// ```rust,no_run
/// use axum::{
///     extract::Path,
///     routing::get,
///     Router,
/// };
/// use std::collections::HashMap;
///
/// async fn params_map(
///     Path(params): Path<HashMap<String, String>>,
/// ) {
///     // ...
/// }
///
/// async fn params_vec(
///     Path(params): Path<Vec<(String, String)>>,
/// ) {
///     // ...
/// }
///
/// let app = Router::new()
///     .route("/users/:user_id/team/:team_id", get(params_map).post(params_vec));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// # Providing detailed rejection output
///
/// If the URI cannot be deserialized into the target type the request will be rejected and an
/// error response will be returned. See [`customize-path-rejection`] for an example of how to customize that error.
///
/// [`serde`]: https://crates.io/crates/serde
/// [`serde::Deserialize`]: https://docs.rs/serde/1.0.127/serde/trait.Deserialize.html
/// [`customize-path-rejection`]: https://github.com/tokio-rs/axum/blob/main/examples/customize-path-rejection/src/main.rs
#[derive(Debug)]
pub struct Path<T>(pub T);

impl<T> Deref for Path<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Path<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl<T, B> FromRequest<B> for Path<T>
where
    T: DeserializeOwned + Send,
    B: Send,
{
    type Rejection = PathRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let params = match req.extensions_mut().get::<UrlParams>() {
            Some(UrlParams::Params(params)) => params,
            Some(UrlParams::InvalidUtf8InPathParam { key }) => {
                let err = PathDeserializationError {
                    kind: ErrorKind::InvalidUtf8InPathParam {
                        key: key.to_string(),
                    },
                };
                let err = FailedToDeserializePathParams(err);
                return Err(err.into());
            }
            None => {
                return Err(MissingPathParams.into());
            }
        };

        T::deserialize(de::PathDeserializer::new(&*params))
            .map_err(|err| {
                PathRejection::FailedToDeserializePathParams(FailedToDeserializePathParams(err))
            })
            .map(Path)
    }
}

// this wrapper type is used as the deserializer error to hide the `serde::de::Error` impl which
// would otherwise be public if we used `ErrorKind` as the error directly
#[derive(Debug)]
pub(crate) struct PathDeserializationError {
    pub(super) kind: ErrorKind,
}

impl PathDeserializationError {
    pub(super) fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    pub(super) fn wrong_number_of_parameters() -> WrongNumberOfParameters<()> {
        WrongNumberOfParameters { got: () }
    }

    pub(super) fn unsupported_type(name: &'static str) -> Self {
        Self::new(ErrorKind::UnsupportedType { name })
    }
}

pub(super) struct WrongNumberOfParameters<G> {
    got: G,
}

impl<G> WrongNumberOfParameters<G> {
    #[allow(clippy::unused_self)]
    pub(super) fn got<G2>(self, got: G2) -> WrongNumberOfParameters<G2> {
        WrongNumberOfParameters { got }
    }
}

impl WrongNumberOfParameters<usize> {
    pub(super) fn expected(self, expected: usize) -> PathDeserializationError {
        PathDeserializationError::new(ErrorKind::WrongNumberOfParameters {
            got: self.got,
            expected,
        })
    }
}

impl serde::de::Error for PathDeserializationError {
    #[inline]
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Self {
            kind: ErrorKind::Message(msg.to_string()),
        }
    }
}

impl fmt::Display for PathDeserializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl std::error::Error for PathDeserializationError {}

/// The kinds of errors that can happen we deserializing into a [`Path`].
///
/// This type is obtained through [`FailedToDeserializePathParams::into_kind`] and is useful for building
/// more precise error messages.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The URI contained the wrong number of parameters.
    WrongNumberOfParameters {
        /// The number of actual parameters in the URI.
        got: usize,
        /// The number of expected parameters.
        expected: usize,
    },

    /// Failed to parse the value at a specific key into the expected type.
    ///
    /// This variant is used when deserializing into types that have named fields, such as structs.
    ParseErrorAtKey {
        /// The key at which the value was located.
        key: String,
        /// The value from the URI.
        value: String,
        /// The expected type of the value.
        expected_type: &'static str,
    },

    /// Failed to parse the value at a specific index into the expected type.
    ///
    /// This variant is used when deserializing into sequence types, such as tuples.
    ParseErrorAtIndex {
        /// The index at which the value was located.
        index: usize,
        /// The value from the URI.
        value: String,
        /// The expected type of the value.
        expected_type: &'static str,
    },

    /// Failed to parse a value into the expected type.
    ///
    /// This variant is used when deserializing into a primitive type (such as `String` and `u32`).
    ParseError {
        /// The value from the URI.
        value: String,
        /// The expected type of the value.
        expected_type: &'static str,
    },

    /// A parameter contained text that, once percent decoded, wasn't valid UTF-8.
    InvalidUtf8InPathParam {
        /// The key at which the invalid value was located.
        key: String,
    },

    /// Tried to serialize into an unsupported type such as nested maps.
    ///
    /// This error kind is caused by programmer errors and thus gets converted into a `500 Internal
    /// Server Error` response.
    UnsupportedType {
        /// The name of the unsupported type.
        name: &'static str,
    },

    /// Catch-all variant for errors that don't fit any other variant.
    Message(String),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Message(error) => error.fmt(f),
            ErrorKind::InvalidUtf8InPathParam { key } => write!(f, "Invalid UTF-8 in `{}`", key),
            ErrorKind::WrongNumberOfParameters { got, expected } => {
                write!(
                    f,
                    "Wrong number of path arguments for `Path`. Expected {} but got {}",
                    expected, got
                )?;

                if *expected == 1 {
                    write!(f, ". Note that multiple parameters must be extracted with a tuple `Path<(_, _)>` or a struct `Path<YourParams>`")?;
                }

                Ok(())
            }
            ErrorKind::UnsupportedType { name } => write!(f, "Unsupported type `{}`", name),
            ErrorKind::ParseErrorAtKey {
                key,
                value,
                expected_type,
            } => write!(
                f,
                "Cannot parse `{}` with value `{:?}` to a `{}`",
                key, value, expected_type
            ),
            ErrorKind::ParseError {
                value,
                expected_type,
            } => write!(f, "Cannot parse `{:?}` to a `{}`", value, expected_type),
            ErrorKind::ParseErrorAtIndex {
                index,
                value,
                expected_type,
            } => write!(
                f,
                "Cannot parse value at index {} with value `{:?}` to a `{}`",
                index, value, expected_type
            ),
        }
    }
}

/// Rejection type for [`Path`](super::Path) if the captured routes params couldn't be deserialized
/// into the expected type.
#[derive(Debug)]
pub struct FailedToDeserializePathParams(PathDeserializationError);

impl FailedToDeserializePathParams {
    /// Convert this error into the underlying error kind.
    pub fn into_kind(self) -> ErrorKind {
        self.0.kind
    }
}

impl IntoResponse for FailedToDeserializePathParams {
    fn into_response(self) -> Response {
        let (status, body) = match self.0.kind {
            ErrorKind::Message(_)
            | ErrorKind::InvalidUtf8InPathParam { .. }
            | ErrorKind::ParseError { .. }
            | ErrorKind::ParseErrorAtIndex { .. }
            | ErrorKind::ParseErrorAtKey { .. } => (
                StatusCode::BAD_REQUEST,
                format!("Invalid URL: {}", self.0.kind),
            ),
            ErrorKind::WrongNumberOfParameters { .. } | ErrorKind::UnsupportedType { .. } => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.0.kind.to_string())
            }
        };
        (status, body).into_response()
    }
}

impl fmt::Display for FailedToDeserializePathParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for FailedToDeserializePathParams {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{routing::get, test_helpers::*, Router};
    use http::{Request, StatusCode};
    use hyper::Body;
    use std::collections::HashMap;

    #[tokio::test]
    async fn extracting_url_params() {
        let app = Router::new().route(
            "/users/:id",
            get(|Path(id): Path<i32>| async move {
                assert_eq!(id, 42);
            })
            .post(|Path(params_map): Path<HashMap<String, i32>>| async move {
                assert_eq!(params_map.get("id").unwrap(), &1337);
            }),
        );

        let client = TestClient::new(app);

        let res = client.get("/users/42").send().await;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client.post("/users/1337").send().await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn extracting_url_params_multiple_times() {
        let app = Router::new().route("/users/:id", get(|_: Path<i32>, _: Path<String>| async {}));

        let client = TestClient::new(app);

        let res = client.get("/users/42").send().await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn percent_decoding() {
        let app = Router::new().route(
            "/:key",
            get(|Path(param): Path<String>| async move { param }),
        );

        let client = TestClient::new(app);

        let res = client.get("/one%20two").send().await;

        assert_eq!(res.text().await, "one two");
    }

    #[tokio::test]
    async fn supports_128_bit_numbers() {
        let app = Router::new()
            .route(
                "/i/:key",
                get(|Path(param): Path<i128>| async move { param.to_string() }),
            )
            .route(
                "/u/:key",
                get(|Path(param): Path<u128>| async move { param.to_string() }),
            );

        let client = TestClient::new(app);

        let res = client.get("/i/123").send().await;
        assert_eq!(res.text().await, "123");

        let res = client.get("/u/123").send().await;
        assert_eq!(res.text().await, "123");
    }

    #[tokio::test]
    async fn wildcard() {
        let app = Router::new()
            .route(
                "/foo/*rest",
                get(|Path(param): Path<String>| async move { param }),
            )
            .route(
                "/bar/*rest",
                get(|Path(params): Path<HashMap<String, String>>| async move {
                    params.get("rest").unwrap().clone()
                }),
            );

        let client = TestClient::new(app);

        let res = client.get("/foo/bar/baz").send().await;
        assert_eq!(res.text().await, "/bar/baz");

        let res = client.get("/bar/baz/qux").send().await;
        assert_eq!(res.text().await, "/baz/qux");
    }

    #[tokio::test]
    async fn captures_dont_match_empty_segments() {
        let app = Router::new().route("/:key", get(|| async {}));

        let client = TestClient::new(app);

        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        let res = client.get("/foo").send().await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn when_extensions_are_missing() {
        let app = Router::new().route("/:key", get(|_: Request<Body>, _: Path<String>| async {}));

        let client = TestClient::new(app);

        let res = client.get("/foo").send().await;
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            res.text().await,
            "No paths parameters found for matched route. Are you also extracting `Request<_>`?"
        );
    }

    #[tokio::test]
    async fn str_reference_deserialize() {
        struct Param(String);
        impl<'de> serde::Deserialize<'de> for Param {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = <&str as serde::Deserialize>::deserialize(deserializer)?;
                Ok(Param(s.to_owned()))
            }
        }

        let app = Router::new().route("/:key", get(|param: Path<Param>| async move { param.0 .0 }));

        let client = TestClient::new(app);

        let res = client.get("/foo").send().await;
        assert_eq!(res.text().await, "foo");

        // percent decoding should also work
        let res = client.get("/foo%20bar").send().await;
        assert_eq!(res.text().await, "foo bar");
    }

    #[tokio::test]
    async fn two_path_extractors() {
        let app = Router::new().route("/:a/:b", get(|_: Path<String>, _: Path<String>| async {}));

        let client = TestClient::new(app);

        let res = client.get("/a/b").send().await;
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            res.text().await,
            "Wrong number of path arguments for `Path`. Expected 1 but got 2. \
            Note that multiple parameters must be extracted with a tuple `Path<(_, _)>` or a struct `Path<YourParams>`",
        );
    }
}
