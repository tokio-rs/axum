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
//! use axum::prelude::*;
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
//! use axum::{async_trait, extract::{FromRequest, RequestParts}, prelude::*};
//! use http::{StatusCode, header::{HeaderValue, USER_AGENT}};
//!
//! struct ExtractUserAgent(HeaderValue);
//!
//! #[async_trait]
//! impl<B> FromRequest<B> for ExtractUserAgent
//! where
//!     B: Send,
//! {
//!     type Rejection = (StatusCode, &'static str);
//!
//!     async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
//!         let user_agent = req.headers().and_then(|headers| headers.get(USER_AGENT));
//!
//!         if let Some(user_agent) = user_agent {
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
//! use axum::prelude::*;
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
//! Note that only one extractor can consume the request body. If multiple body extractors are
//! applied a `500 Internal Server Error` response will be returned.
//!
//! # Optional extractors
//!
//! Wrapping extractors in `Option` will make them optional:
//!
//! ```rust,no_run
//! use axum::{extract::Json, prelude::*};
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
//! use axum::{extract::{Json, rejection::JsonRejection}, prelude::*};
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
//! use axum::{extract::Json, prelude::*};
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
//!
//! # Request body extractors
//!
//! Most of the time your request body type will be [`body::Body`] (a re-export
//! of [`hyper::Body`]), which is directly supported by all extractors.
//!
//! However if you're applying a tower middleware that changes the response you
//! might have to apply a different body type to some extractors:
//!
//! ```rust
//! use std::{
//!     task::{Context, Poll},
//!     pin::Pin,
//! };
//! use tower_http::map_request_body::MapRequestBodyLayer;
//! use axum::prelude::*;
//!
//! struct MyBody<B>(B);
//!
//! impl<B> http_body::Body for MyBody<B>
//! where
//!     B: http_body::Body + Unpin,
//! {
//!     type Data = B::Data;
//!     type Error = B::Error;
//!
//!     fn poll_data(
//!         mut self: Pin<&mut Self>,
//!         cx: &mut Context<'_>,
//!     ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
//!         Pin::new(&mut self.0).poll_data(cx)
//!     }
//!
//!     fn poll_trailers(
//!         mut self: Pin<&mut Self>,
//!         cx: &mut Context<'_>,
//!     ) -> Poll<Result<Option<headers::HeaderMap>, Self::Error>> {
//!         Pin::new(&mut self.0).poll_trailers(cx)
//!     }
//! }
//!
//! let app =
//!     // `String` works directly with any body type
//!     route(
//!         "/string",
//!         get(|_: String| async {})
//!     )
//!     .route(
//!         "/body",
//!         // `extract::Body` defaults to `axum::body::Body`
//!         // but can be customized
//!         get(|_: extract::Body<MyBody<Body>>| async {})
//!     )
//!     .route(
//!         "/body-stream",
//!         // same for `extract::BodyStream`
//!         get(|_: extract::BodyStream<MyBody<Body>>| async {}),
//!     )
//!     .route(
//!         // and `Request<_>`
//!         "/request",
//!         get(|_: Request<MyBody<Body>>| async {})
//!     )
//!     // middleware that changes the request body type
//!     .layer(MapRequestBodyLayer::new(MyBody));
//! # async {
//! # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! [`body::Body`]: crate::body::Body

use crate::{response::IntoResponse, util::ByteStr};
use async_trait::async_trait;
use bytes::{Buf, Bytes};
use futures_util::stream::Stream;
use http::{header, Extensions, HeaderMap, Method, Request, Response, Uri, Version};
use rejection::*;
use serde::de::DeserializeOwned;
use std::{
    collections::HashMap,
    convert::Infallible,
    pin::Pin,
    str::FromStr,
    task::{Context, Poll},
};

pub mod extractor_middleware;
pub mod rejection;

#[doc(inline)]
pub use self::extractor_middleware::extractor_middleware;

#[cfg(feature = "multipart")]
#[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
pub mod multipart;

#[cfg(feature = "multipart")]
#[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
#[doc(inline)]
pub use self::multipart::Multipart;

/// Types that can be created from requests.
///
/// See the [module docs](crate::extract) for more details.
#[async_trait]
pub trait FromRequest<B>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection>;
}

/// The type used with [`FromRequest`] to extract data from requests.
///
/// Has several convenience methods for getting owned parts of the request.
#[derive(Debug)]
pub struct RequestParts<B> {
    method: Option<Method>,
    uri: Option<Uri>,
    version: Option<Version>,
    headers: Option<HeaderMap>,
    extensions: Option<Extensions>,
    body: Option<B>,
}

impl<B> RequestParts<B> {
    pub(crate) fn new(req: Request<B>) -> Self {
        let (
            http::request::Parts {
                method,
                uri,
                version,
                headers,
                extensions,
                ..
            },
            body,
        ) = req.into_parts();

        RequestParts {
            method: Some(method),
            uri: Some(uri),
            version: Some(version),
            headers: Some(headers),
            extensions: Some(extensions),
            body: Some(body),
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn into_request(&mut self) -> Request<B> {
        let Self {
            method,
            uri,
            version,
            headers,
            extensions,
            body,
        } = self;

        let mut req = Request::new(body.take().expect("body already extracted"));

        if let Some(method) = method.take() {
            *req.method_mut() = method;
        }

        if let Some(uri) = uri.take() {
            *req.uri_mut() = uri;
        }

        if let Some(version) = version.take() {
            *req.version_mut() = version;
        }

        if let Some(headers) = headers.take() {
            *req.headers_mut() = headers;
        }

        if let Some(extensions) = extensions.take() {
            *req.extensions_mut() = extensions;
        }

        req
    }

    /// Gets a reference to the request method.
    ///
    /// Returns `None` if the method has been taken by another extractor.
    pub fn method(&self) -> Option<&Method> {
        self.method.as_ref()
    }

    /// Gets a mutable reference to the request method.
    ///
    /// Returns `None` if the method has been taken by another extractor.
    pub fn method_mut(&mut self) -> Option<&mut Method> {
        self.method.as_mut()
    }

    /// Takes the method out of the request, leaving a `None` in its place.
    pub fn take_method(&mut self) -> Option<Method> {
        self.method.take()
    }

    /// Gets a reference to the request URI.
    ///
    /// Returns `None` if the URI has been taken by another extractor.
    pub fn uri(&self) -> Option<&Uri> {
        self.uri.as_ref()
    }

    /// Gets a mutable reference to the request URI.
    ///
    /// Returns `None` if the URI has been taken by another extractor.
    pub fn uri_mut(&mut self) -> Option<&mut Uri> {
        self.uri.as_mut()
    }

    /// Takes the URI out of the request, leaving a `None` in its place.
    pub fn take_uri(&mut self) -> Option<Uri> {
        self.uri.take()
    }

    /// Gets a reference to the request HTTP version.
    ///
    /// Returns `None` if the HTTP version has been taken by another extractor.
    pub fn version(&self) -> Option<Version> {
        self.version
    }

    /// Gets a mutable reference to the request HTTP version.
    ///
    /// Returns `None` if the HTTP version has been taken by another extractor.
    pub fn version_mut(&mut self) -> Option<&mut Version> {
        self.version.as_mut()
    }

    /// Takes the HTTP version out of the request, leaving a `None` in its place.
    pub fn take_version(&mut self) -> Option<Version> {
        self.version.take()
    }

    /// Gets a reference to the request headers.
    ///
    /// Returns `None` if the headers has been taken by another extractor.
    pub fn headers(&self) -> Option<&HeaderMap> {
        self.headers.as_ref()
    }

    /// Gets a mutable reference to the request headers.
    ///
    /// Returns `None` if the headers has been taken by another extractor.
    pub fn headers_mut(&mut self) -> Option<&mut HeaderMap> {
        self.headers.as_mut()
    }

    /// Takes the headers out of the request, leaving a `None` in its place.
    pub fn take_headers(&mut self) -> Option<HeaderMap> {
        self.headers.take()
    }

    /// Gets a reference to the request extensions.
    ///
    /// Returns `None` if the extensions has been taken by another extractor.
    pub fn extensions(&self) -> Option<&Extensions> {
        self.extensions.as_ref()
    }

    /// Gets a mutable reference to the request extensions.
    ///
    /// Returns `None` if the extensions has been taken by another extractor.
    pub fn extensions_mut(&mut self) -> Option<&mut Extensions> {
        self.extensions.as_mut()
    }

    /// Takes the extensions out of the request, leaving a `None` in its place.
    pub fn take_extensions(&mut self) -> Option<Extensions> {
        self.extensions.take()
    }

    /// Gets a reference to the request body.
    ///
    /// Returns `None` if the body has been taken by another extractor.
    pub fn body(&self) -> Option<&B> {
        self.body.as_ref()
    }

    /// Gets a mutable reference to the request body.
    ///
    /// Returns `None` if the body has been taken by another extractor.
    pub fn body_mut(&mut self) -> Option<&mut B> {
        self.body.as_mut()
    }

    /// Takes the body out of the request, leaving a `None` in its place.
    pub fn take_body(&mut self) -> Option<B> {
        self.body.take()
    }
}

#[async_trait]
impl<B> FromRequest<B> for ()
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(_: &mut RequestParts<B>) -> Result<(), Self::Rejection> {
        Ok(())
    }
}

macro_rules! impl_from_request {
    () => {
    };

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<B, $head, $($tail,)*> FromRequest<B> for ($head, $($tail,)*)
        where
            $head: FromRequest<B> + Send,
            $( $tail: FromRequest<B> + Send, )*
            B: Send,
        {
            type Rejection = Response<crate::body::Body>;

            async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
                let $head = $head::from_request(req).await.map_err(IntoResponse::into_response)?;
                $( let $tail = $tail::from_request(req).await.map_err(IntoResponse::into_response)?; )*
                Ok(($head, $($tail,)*))
            }
        }

        impl_from_request!($($tail,)*);
    };
}

impl_from_request!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

#[async_trait]
impl<T, B> FromRequest<B> for Option<T>
where
    T: FromRequest<B>,
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Option<T>, Self::Rejection> {
        Ok(T::from_request(req).await.ok())
    }
}

#[async_trait]
impl<T, B> FromRequest<B> for Result<T, T::Rejection>
where
    T: FromRequest<B>,
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
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
/// use axum::prelude::*;
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
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If the query string cannot be parsed it will reject the request with a `400
/// Bad Request` response.
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for Query<T>
where
    T: DeserializeOwned,
    B: Send,
{
    type Rejection = QueryRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let query = req
            .uri()
            .ok_or(UriAlreadyExtracted)?
            .query()
            .ok_or(QueryStringMissing)?;
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
/// use axum::prelude::*;
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
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Note that `Content-Type: multipart/form-data` requests are not supported.
#[derive(Debug, Clone, Copy, Default)]
pub struct Form<T>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for Form<T>
where
    T: DeserializeOwned,
    B: http_body::Body + Send,
    B::Data: Send,
    B::Error: Into<tower::BoxError>,
{
    type Rejection = FormRejection;

    #[allow(warnings)]
    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if !has_content_type(&req, "application/x-www-form-urlencoded")? {
            Err(InvalidFormContentType)?;
        }

        if req.method().ok_or(MethodAlreadyExtracted)? == Method::GET {
            let query = req
                .uri()
                .ok_or(UriAlreadyExtracted)?
                .query()
                .ok_or(QueryStringMissing)?;
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
/// use axum::prelude::*;
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
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If the query string cannot be parsed it will reject the request with a `400
/// Bad Request` response.
///
/// The request is required to have a `Content-Type: application/json` header.
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for Json<T>
where
    T: DeserializeOwned,
    B: http_body::Body + Send,
    B::Data: Send,
    B::Error: Into<tower::BoxError>,
{
    type Rejection = JsonRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        use bytes::Buf;

        if has_content_type(req, "application/json")? {
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

fn has_content_type<B>(
    req: &RequestParts<B>,
    expected_content_type: &str,
) -> Result<bool, HeadersAlreadyExtracted> {
    let content_type = if let Some(content_type) = req
        .headers()
        .ok_or(HeadersAlreadyExtracted)?
        .get(header::CONTENT_TYPE)
    {
        content_type
    } else {
        return Ok(false);
    };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return Ok(false);
    };

    Ok(content_type.starts_with(expected_content_type))
}

/// Extractor that gets a value from request extensions.
///
/// This is commonly used to share state across handlers.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{AddExtensionLayer, prelude::*};
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
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If the extension is missing it will reject the request with a `500 Interal
/// Server Error` response.
#[derive(Debug, Clone, Copy)]
pub struct Extension<T>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for Extension<T>
where
    T: Clone + Send + Sync + 'static,
    B: Send,
{
    type Rejection = ExtensionRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let value = req
            .extensions()
            .ok_or(ExtensionsAlreadyExtracted)?
            .get::<T>()
            .ok_or(MissingExtension)
            .map(|x| x.clone())?;

        Ok(Extension(value))
    }
}

#[async_trait]
impl<B> FromRequest<B> for Bytes
where
    B: http_body::Body + Send,
    B::Data: Send,
    B::Error: Into<tower::BoxError>,
{
    type Rejection = BytesRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)?;

        Ok(bytes)
    }
}

#[async_trait]
impl<B> FromRequest<B> for String
where
    B: http_body::Body + Send,
    B::Data: Send,
    B::Error: Into<tower::BoxError>,
{
    type Rejection = StringRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;

        let bytes = hyper::body::to_bytes(body)
            .await
            .map_err(FailedToBufferBody::from_err)?
            .to_vec();

        let string = String::from_utf8(bytes).map_err(InvalidUtf8::from_err)?;

        Ok(string)
    }
}

/// Extractor that extracts the request body as a [`Stream`].
///
/// # Example
///
/// ```rust,no_run
/// use axum::prelude::*;
/// use futures::StreamExt;
///
/// async fn handler(mut stream: extract::BodyStream) {
///     while let Some(chunk) = stream.next().await {
///         // ...
///     }
/// }
///
/// let app = route("/users", get(handler));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[derive(Debug)]
pub struct BodyStream<B = crate::body::Body>(B);

impl<B> Stream for BodyStream<B>
where
    B: http_body::Body + Unpin,
{
    type Item = Result<B::Data, B::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.0).poll_data(cx)
    }
}

#[async_trait]
impl<B> FromRequest<B> for BodyStream<B>
where
    B: http_body::Body + Unpin + Send,
{
    type Rejection = BodyAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;
        let stream = BodyStream(body);
        Ok(stream)
    }
}

/// Extractor that extracts the request body.
///
/// # Example
///
/// ```rust,no_run
/// use axum::prelude::*;
/// use futures::StreamExt;
///
/// async fn handler(extract::Body(body): extract::Body) {
///     // ...
/// }
///
/// let app = route("/users", get(handler));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[derive(Debug, Default, Clone)]
pub struct Body<B = crate::body::Body>(pub B);

#[async_trait]
impl<B> FromRequest<B> for Body<B>
where
    B: Send,
{
    type Rejection = BodyAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let body = take_body(req)?;
        Ok(Self(body))
    }
}

#[async_trait]
impl<B> FromRequest<B> for Request<B>
where
    B: Send,
{
    type Rejection = RequestAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let RequestParts {
            method,
            uri,
            version,
            headers,
            extensions,
            body,
        } = req;

        let all_parts = method
            .as_ref()
            .zip(version.as_ref())
            .zip(uri.as_ref())
            .zip(extensions.as_ref())
            .zip(body.as_ref())
            .zip(headers.as_ref());

        if all_parts.is_some() {
            Ok(req.into_request())
        } else {
            Err(RequestAlreadyExtracted)
        }
    }
}

#[async_trait]
impl<B> FromRequest<B> for Method
where
    B: Send,
{
    type Rejection = MethodAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        req.take_method().ok_or(MethodAlreadyExtracted)
    }
}

#[async_trait]
impl<B> FromRequest<B> for Uri
where
    B: Send,
{
    type Rejection = UriAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        req.take_uri().ok_or(UriAlreadyExtracted)
    }
}

#[async_trait]
impl<B> FromRequest<B> for Version
where
    B: Send,
{
    type Rejection = VersionAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        req.take_version().ok_or(VersionAlreadyExtracted)
    }
}

#[async_trait]
impl<B> FromRequest<B> for HeaderMap
where
    B: Send,
{
    type Rejection = HeadersAlreadyExtracted;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        req.take_headers().ok_or(HeadersAlreadyExtracted)
    }
}

/// Extractor that will reject requests with a body larger than some size.
///
/// # Example
///
/// ```rust,no_run
/// use axum::prelude::*;
///
/// async fn handler(body: extract::ContentLengthLimit<String, 1024>) {
///     // ...
/// }
///
/// let app = route("/", post(handler));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// This requires the request to have a `Content-Length` header.
#[derive(Debug, Clone)]
pub struct ContentLengthLimit<T, const N: u64>(pub T);

#[async_trait]
impl<T, B, const N: u64> FromRequest<B> for ContentLengthLimit<T, N>
where
    T: FromRequest<B>,
    B: Send,
{
    type Rejection = ContentLengthLimitRejection<T::Rejection>;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let content_length = req
            .headers()
            .ok_or(ContentLengthLimitRejection::HeadersAlreadyExtracted(
                HeadersAlreadyExtracted,
            ))?
            .get(http::header::CONTENT_LENGTH);

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
/// use axum::prelude::*;
///
/// async fn users_show(params: extract::UrlParamsMap) {
///     let id: Option<&str> = params.get("id");
///
///     // ...
/// }
///
/// let app = route("/users/:id", get(users_show));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
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
impl<B> FromRequest<B> for UrlParamsMap
where
    B: Send,
{
    type Rejection = MissingRouteParams;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if let Some(params) = req
            .extensions_mut()
            .and_then(|ext| ext.get_mut::<Option<crate::routing::UrlParams>>())
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
/// use axum::{extract::UrlParams, prelude::*};
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
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
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
        impl<B, $head, $($tail,)*> FromRequest<B> for UrlParams<($head, $($tail,)*)>
        where
            $head: FromStr + Send,
            $( $tail: FromStr + Send, )*
            B: Send,
        {
            type Rejection = UrlParamsRejection;

            #[allow(non_snake_case)]
            async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
                let params = if let Some(params) = req
                    .extensions_mut()
                    .and_then(|ext| {
                        ext.get_mut::<Option<crate::routing::UrlParams>>()
                    })
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

fn take_body<B>(req: &mut RequestParts<B>) -> Result<B, BodyAlreadyExtracted> {
    req.take_body().ok_or(BodyAlreadyExtracted)
}

/// Extractor that extracts a typed header value from [`headers`].
///
/// # Example
///
/// ```rust,no_run
/// use axum::{extract::TypedHeader, prelude::*};
/// use headers::UserAgent;
///
/// async fn users_teams_show(
///     TypedHeader(user_agent): TypedHeader<UserAgent>,
/// ) {
///     // ...
/// }
///
/// let app = route("/users/:user_id/team/:team_id", get(users_teams_show));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
#[derive(Debug, Clone, Copy)]
pub struct TypedHeader<T>(pub T);

#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
#[async_trait]
impl<T, B> FromRequest<B> for TypedHeader<T>
where
    T: headers::Header,
    B: Send,
{
    type Rejection = TypedHeaderRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let empty_headers = HeaderMap::new();
        let header_values = if let Some(headers) = req.headers() {
            headers.get_all(T::name())
        } else {
            empty_headers.get_all(T::name())
        };

        T::decode(&mut header_values.iter())
            .map(Self)
            .map_err(|err| rejection::TypedHeaderRejection {
                err,
                name: T::name(),
            })
    }
}

/// Extractor that extracts the raw query string, without parsing it.
///
/// # Example
///
/// ```rust,no_run
/// use axum::prelude::*;
/// use futures::StreamExt;
///
/// async fn handler(extract::RawQuery(query): extract::RawQuery) {
///     // ...
/// }
///
/// let app = route("/users", get(handler));
/// # async {
/// # hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[derive(Debug)]
pub struct RawQuery(pub Option<String>);

#[async_trait]
impl<B> FromRequest<B> for RawQuery
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let query = req
            .uri()
            .and_then(|uri| uri.query())
            .map(|query| query.to_string());
        Ok(Self(query))
    }
}
