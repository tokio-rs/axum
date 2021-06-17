//! Types and traits for extracting data from requests.
//!
//! A handler function is an async function take takes any number of
//! "extractors" as arguments. An extractor is a type that implements
//! [`FromRequest`](crate::extract::FromRequest).
//!
//! For example, [`Json`] is an extractor that consumes the request body and
//! deserializes it as JSON into some target type:
//!
//! ```rust,no_run
//! use awebframework::prelude::*;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct CreateUser {
//!     email: String,
//!     password: String,
//! }
//!
//! async fn create_user(payload: extract::Json<CreateUser>) {
//!     let payload: CreateUser = payload.0;
//!
//!     // ...
//! }
//!
//! let app = route("/users", post(create_user));
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! # Defining custom extractors
//!
//! You can also define your own extractors by implementing [`FromRequest`]:
//!
//! ```rust,no_run
//! use awebframework::{async_trait, extract::FromRequest, prelude::*};
//! use http::{StatusCode, header::{HeaderValue, USER_AGENT}};
//!
//! struct ExtractUserAgent(HeaderValue);
//!
//! #[async_trait]
//! impl FromRequest for ExtractUserAgent {
//!     type Rejection = (StatusCode, &'static str);
//!
//!     async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
//!         if let Some(user_agent) = req.headers().get(USER_AGENT) {
//!             Ok(ExtractUserAgent(user_agent.clone()))
//!         } else {
//!             Err((StatusCode::BAD_REQUEST, "`User-Agent` header is missing"))
//!         }
//!     }
//! }
//!
//! async fn handler(user_agent: ExtractUserAgent) {
//!     let user_agent: HeaderValue = user_agent.0;
//!
//!     // ...
//! }
//!
//! let app = route("/foo", get(handler));
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! # Multiple extractors
//!
//! Handlers can also contain multiple extractors:
//!
//! ```rust,no_run
//! use awebframework::prelude::*;
//! use std::collections::HashMap;
//!
//! async fn handler(
//!     // Extract captured parameters from the URL
//!     params: extract::UrlParamsMap,
//!     // Parse query string into a `HashMap`
//!     query_params: extract::Query<HashMap<String, String>>,
//!     // Buffer the request body into a `Bytes`
//!     bytes: bytes::Bytes,
//! ) {
//!     // ...
//! }
//!
//! let app = route("/foo", get(handler));
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! # Optional extractors
//!
//! Wrapping extractors in `Option` will make them optional:
//!
//! ```rust,no_run
//! use awebframework::{extract::Json, prelude::*};
//! use serde_json::Value;
//!
//! async fn create_user(payload: Option<Json<Value>>) {
//!     if let Some(payload) = payload {
//!         // We got a valid JSON payload
//!     } else {
//!         // Payload wasn't valid JSON
//!     }
//! }
//!
//! let app = route("/users", post(create_user));
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! Wrapping extractors in `Result` makes them optional and gives you the reason
//! the extraction failed:
//!
//! ```rust,no_run
//! use awebframework::{extract::{Json, rejection::JsonRejection}, prelude::*};
//! use serde_json::Value;
//!
//! async fn create_user(payload: Result<Json<Value>, JsonRejection>) {
//!     match payload {
//!         Ok(payload) => {
//!             // We got a valid JSON payload
//!         }
//!         Err(JsonRejection::MissingJsonContentType(_)) => {
//!             // Request didn't have `Content-Type: application/json`
//!             // header
//!         }
//!         Err(JsonRejection::InvalidJsonBody(_)) => {
//!             // Couldn't deserialize the body into the target type
//!         }
//!         Err(JsonRejection::BodyAlreadyExtracted(_)) => {
//!             // Another extractor had already consumed the body
//!         }
//!         Err(_) => {
//!             // `JsonRejection` is marked `#[non_exhaustive]` so match must
//!             // include a catch-all case.
//!         }
//!     }
//! }
//!
//! let app = route("/users", post(create_user));
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! # Reducing boilerplate
//!
//! If you're feeling adventorous you can even deconstruct the extractors
//! directly on the function signature:
//!
//! ```rust,no_run
//! use awebframework::{extract::Json, prelude::*};
//! use serde_json::Value;
//!
//! async fn create_user(Json(value): Json<Value>) {
//!     // `value` is of type `Value`
//! }
//!
//! let app = route("/users", post(create_user));
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```

use crate::{body::Body, response::IntoResponse, util::ByteStr};
use async_trait::async_trait;
use bytes::{Buf, Bytes};
use http::{header, HeaderMap, Method, Request, Uri, Version};
use rejection::*;
use serde::de::DeserializeOwned;
use std::{collections::HashMap, convert::Infallible, mem, str::FromStr};

pub mod rejection;

/// Types that can be created from requests.
///
/// See the [module docs](crate::extract) for more details.
#[async_trait]
pub trait FromRequest: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection>;
}

#[async_trait]
impl<T> FromRequest for Option<T>
where
    T: FromRequest,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut Request<Body>) -> Result<Option<T>, Self::Rejection> {
        Ok(T::from_request(req).await.ok())
    }
}

#[async_trait]
impl<T> FromRequest for Result<T, T::Rejection>
where
    T: FromRequest,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        Ok(T::from_request(req).await)
    }
}

/// Extractor that deserializes query strings into some type.
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Example
///
/// ```rust,no_run
/// use awebframework::prelude::*;
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
/// async fn list_things(pagination: extract::Query<Pagination>) {
///     let pagination: Pagination = pagination.0;
///
///     // ...
/// }
///
/// let app = route("/list_things", get(list_things));
/// ```
///
/// If the query string cannot be parsed it will reject the request with a `400
/// Bad Request` response.
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

#[async_trait]
impl<T> FromRequest for Query<T>
where
    T: DeserializeOwned,
{
    type Rejection = QueryRejection;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        let query = req.uri().query().ok_or(QueryStringMissing)?;
        let value = serde_urlencoded::from_str(query)
            .map_err(FailedToDeserializeQueryString::new::<T, _>)?;
        Ok(Query(value))
    }
}

/// Extractor that deserializes `application/x-www-form-urlencoded` requests
/// into some type.
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Example
///
/// ```rust,no_run
/// use awebframework::prelude::*;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct SignUp {
///     username: String,
///     password: String,
/// }
///
/// async fn accept_form(form: extract::Form<SignUp>) {
///     let sign_up: SignUp = form.0;
///
///     // ...
/// }
///
/// let app = route("/sign_up", post(accept_form));
/// ```
///
/// Note that `Content-Type: multipart/form-data` requests are not supported.
#[derive(Debug, Clone, Copy, Default)]
pub struct Form<T>(pub T);

#[async_trait]
impl<T> FromRequest for Form<T>
where
    T: DeserializeOwned,
{
    type Rejection = FormRejection;

    #[allow(warnings)]
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        if !has_content_type(&req, "application/x-www-form-urlencoded") {
            Err(InvalidFormContentType)?;
        }

        if req.method() == Method::GET {
            let query = req.uri().query().ok_or(QueryStringMissing)?;
            let value = serde_urlencoded::from_str(query)
                .map_err(FailedToDeserializeQueryString::new::<T, _>)?;
            Ok(Form(value))
        } else {
            let body = take_body(req)?;
            let chunks = hyper::body::aggregate(body)
                .await
                .map_err(FailedToBufferBody::from_err)?;
            let value = serde_urlencoded::from_reader(chunks.reader())
                .map_err(FailedToDeserializeQueryString::new::<T, _>)?;

            Ok(Form(value))
        }
    }
}

/// Extractor that deserializes request bodies into some type.
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Example
///
/// ```rust,no_run
/// use awebframework::prelude::*;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct CreateUser {
///     email: String,
///     password: String,
/// }
///
/// async fn create_user(payload: extract::Json<CreateUser>) {
///     let payload: CreateUser = payload.0;
///
///     // ...
/// }
///
/// let app = route("/users", post(create_user));
/// ```
///
/// If the query string cannot be parsed it will reject the request with a `400
/// Bad Request` response.
///
/// The request is required to have a `Content-Type: application/json` header.
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

#[async_trait]
impl<T> FromRequest for Json<T>
where
    T: DeserializeOwned,
{
    type Rejection = JsonRejection;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        use bytes::Buf;

        if has_content_type(req, "application/json") {
            let body = take_body(req)?;

            let buf = hyper::body::aggregate(body)
                .await
                .map_err(InvalidJsonBody::from_err)?;

            let value = serde_json::from_reader(buf.reader()).map_err(InvalidJsonBody::from_err)?;

            Ok(Json(value))
        } else {
            Err(MissingJsonContentType.into())
        }
    }
}

fn has_content_type<B>(req: &Request<B>, expected_content_type: &str) -> bool {
    let content_type = if let Some(content_type) = req.headers().get(header::CONTENT_TYPE) {
        content_type
    } else {
        return false;
    };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return false;
    };

    content_type.starts_with(expected_content_type)
}

/// Extractor that gets a value from request extensions.
///
/// This is commonly used to share state across handlers.
///
/// # Example
///
/// ```rust,no_run
/// use awebframework::{AddExtensionLayer, prelude::*};
/// use std::sync::Arc;
///
/// // Some shared state used throughout our application
/// struct State {
///     // ...
/// }
///
/// async fn handler(state: extract::Extension<Arc<State>>) {
///     // ...
/// }
///
/// let state = Arc::new(State { /* ... */ });
///
/// let app = route("/", get(handler))
///     // Add middleware that inserts the state into all incoming request's
///     // extensions.
///     .layer(AddExtensionLayer::new(state));
/// ```
///
/// If the extension is missing it will reject the request with a `500 Interal
/// Server Error` response.
#[derive(Debug, Clone, Copy)]
pub struct Extension<T>(pub T);

#[async_trait]
impl<T> FromRequest for Extension<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Rejection = MissingExtension;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        let value = req
            .extensions()
            .get::<T>()
            .ok_or(MissingExtension)
            .map(|x| x.clone())?;

        Ok(Extension(value))
    }
}

#[async_trait]
impl FromRequest for Bytes {
    type Rejection = BytesRejection;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)?;

        Ok(bytes)
    }
}

#[async_trait]
impl FromRequest for String {
    type Rejection = StringRejection;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)?
            .to_vec();

        let string = String::from_utf8(bytes).map_err(InvalidUtf8::from_err)?;

        Ok(string)
    }
}

#[async_trait]
impl FromRequest for Body {
    type Rejection = BodyAlreadyExtracted;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        take_body(req)
    }
}

#[async_trait]
impl FromRequest for Request<Body> {
    type Rejection = RequestAlreadyExtracted;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        struct RequestAlreadyExtractedExt;

        if req
            .extensions_mut()
            .insert(RequestAlreadyExtractedExt)
            .is_some()
        {
            Err(RequestAlreadyExtracted)
        } else {
            Ok(mem::take(req))
        }
    }
}

#[async_trait]
impl FromRequest for Method {
    type Rejection = Infallible;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        Ok(req.method().clone())
    }
}

#[async_trait]
impl FromRequest for Uri {
    type Rejection = Infallible;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        Ok(req.uri().clone())
    }
}

#[async_trait]
impl FromRequest for Version {
    type Rejection = Infallible;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        Ok(req.version())
    }
}

#[async_trait]
impl FromRequest for HeaderMap {
    type Rejection = Infallible;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        Ok(mem::take(req.headers_mut()))
    }
}

/// Extractor that will reject requests with a body larger than some size.
///
/// # Example
///
/// ```rust,no_run
/// use awebframework::prelude::*;
///
/// async fn handler(body: extract::ContentLengthLimit<String, 1024>) {
///     // ...
/// }
///
/// let app = route("/", post(handler));
/// ```
///
/// This requires the request to have a `Content-Length` header.
#[derive(Debug, Clone)]
pub struct ContentLengthLimit<T, const N: u64>(pub T);

#[async_trait]
impl<T, const N: u64> FromRequest for ContentLengthLimit<T, N>
where
    T: FromRequest,
{
    type Rejection = ContentLengthLimitRejection<T::Rejection>;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        let content_length = req.headers().get(http::header::CONTENT_LENGTH).cloned();

        let content_length =
            content_length.and_then(|value| value.to_str().ok()?.parse::<u64>().ok());

        if let Some(length) = content_length {
            if length > N {
                return Err(ContentLengthLimitRejection::PayloadTooLarge(
                    PayloadTooLarge,
                ));
            }
        } else {
            return Err(ContentLengthLimitRejection::LengthRequired(LengthRequired));
        };

        let value = T::from_request(req)
            .await
            .map_err(ContentLengthLimitRejection::Inner)?;

        Ok(Self(value))
    }
}

/// Extractor that will get captures from the URL.
///
/// # Example
///
/// ```rust,no_run
/// use awebframework::prelude::*;
///
/// async fn users_show(params: extract::UrlParamsMap) {
///     let id: Option<&str> = params.get("id");
///
///     // ...
/// }
///
/// let app = route("/users/:id", get(users_show));
/// ```
///
/// Note that you can only have one URL params extractor per handler. If you
/// have multiple it'll response with `500 Internal Server Error`.
#[derive(Debug)]
pub struct UrlParamsMap(HashMap<ByteStr, ByteStr>);

impl UrlParamsMap {
    /// Look up the value for a key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(&ByteStr::new(key)).map(|s| s.as_str())
    }

    /// Look up the value for a key and parse it into a value of type `T`.
    pub fn get_typed<T>(&self, key: &str) -> Option<Result<T, T::Err>>
    where
        T: FromStr,
    {
        self.get(key).map(str::parse)
    }
}

#[async_trait]
impl FromRequest for UrlParamsMap {
    type Rejection = MissingRouteParams;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        if let Some(params) = req
            .extensions_mut()
            .get_mut::<Option<crate::routing::UrlParams>>()
        {
            if let Some(params) = params {
                Ok(Self(params.0.iter().cloned().collect()))
            } else {
                Ok(Self(Default::default()))
            }
        } else {
            Err(MissingRouteParams)
        }
    }
}

/// Extractor that will get captures from the URL and parse them.
///
/// # Example
///
/// ```rust,no_run
/// use awebframework::{extract::UrlParams, prelude::*};
/// use uuid::Uuid;
///
/// async fn users_teams_show(
///     UrlParams(params): UrlParams<(Uuid, Uuid)>,
/// ) {
///     let user_id: Uuid = params.0;
///     let team_id: Uuid = params.1;
///
///     // ...
/// }
///
/// let app = route("/users/:user_id/team/:team_id", get(users_teams_show));
/// ```
///
/// Note that you can only have one URL params extractor per handler. If you
/// have multiple it'll response with `500 Internal Server Error`.
#[derive(Debug)]
pub struct UrlParams<T>(pub T);

macro_rules! impl_parse_url {
    () => {};

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        impl<$head, $($tail,)*> FromRequest for UrlParams<($head, $($tail,)*)>
        where
            $head: FromStr + Send,
            $( $tail: FromStr + Send, )*
        {
            type Rejection = UrlParamsRejection;

            #[allow(non_snake_case)]
            async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
                let params = if let Some(params) = req
                    .extensions_mut()
                    .get_mut::<Option<crate::routing::UrlParams>>()
                {
                    if let Some(params) = params {
                        params.0.clone()
                    } else {
                        Default::default()
                    }
                } else {
                    return Err(MissingRouteParams.into())
                };

                if let [(_, $head), $((_, $tail),)*] = &*params {
                    let $head = if let Ok(x) = $head.as_str().parse::<$head>() {
                       x
                    } else {
                        return Err(InvalidUrlParam::new::<$head>().into());
                    };

                    $(
                        let $tail = if let Ok(x) = $tail.as_str().parse::<$tail>() {
                           x
                        } else {
                            return Err(InvalidUrlParam::new::<$tail>().into());
                        };
                    )*

                    Ok(UrlParams(($head, $($tail,)*)))
                } else {
                    Err(MissingRouteParams.into())
                }
            }
        }

        impl_parse_url!($($tail,)*);
    };
}

impl_parse_url!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

fn take_body(req: &mut Request<Body>) -> Result<Body, BodyAlreadyExtracted> {
    struct BodyAlreadyExtractedExt;

    if req
        .extensions_mut()
        .insert(BodyAlreadyExtractedExt)
        .is_some()
    {
        Err(BodyAlreadyExtracted)
    } else {
        Ok(mem::take(req.body_mut()))
    }
}

macro_rules! impl_from_request_tuple {
    () => {};

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        #[async_trait]
        impl<R, $head, $($tail,)*> FromRequest for ($head, $($tail,)*)
        where
            R: IntoResponse,
            $head: FromRequest<Rejection = R> + Send,
            $( $tail: FromRequest<Rejection = R> + Send, )*
        {
            type Rejection = R;

            async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
                let $head = FromRequest::from_request(req).await?;
                $( let $tail = FromRequest::from_request(req).await?; )*
                Ok(($head, $($tail,)*))
            }
        }

        impl_from_request_tuple!($($tail,)*);
    };
}

impl_from_request_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

/// Extractor that extracts a typed header value from [`headers`].
///
/// # Example
///
/// ```rust,no_run
/// use awebframework::{extract::TypedHeader, prelude::*};
/// use headers::UserAgent;
///
/// async fn users_teams_show(
///     TypedHeader(user_agent): TypedHeader<UserAgent>,
/// ) {
///     // ...
/// }
///
/// let app = route("/users/:user_id/team/:team_id", get(users_teams_show));
/// ```
#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
#[derive(Debug, Clone, Copy)]
pub struct TypedHeader<T>(pub T);

#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
#[async_trait]
impl<T> FromRequest for TypedHeader<T>
where
    T: headers::Header,
{
    type Rejection = rejection::TypedHeaderRejection;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection> {
        let header_values = req.headers().get_all(T::name());
        T::decode(&mut header_values.iter())
            .map(Self)
            .map_err(|err| rejection::TypedHeaderRejection {
                err,
                name: T::name(),
            })
    }
}
