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
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
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
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
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
//!     params: extract::Path<HashMap<String, String>>,
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
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
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
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
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
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
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
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
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
//! # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
//! # };
//! ```
//!
//! [`body::Body`]: crate::body::Body

use crate::{response::IntoResponse, Error};
use async_trait::async_trait;
use http::{header, Extensions, HeaderMap, Method, Request, Uri, Version};
use rejection::*;
use std::convert::Infallible;

pub mod connect_info;
pub mod extractor_middleware;
pub mod rejection;

#[cfg(feature = "ws")]
#[cfg_attr(docsrs, doc(cfg(feature = "ws")))]
pub mod ws;

mod content_length_limit;
mod extension;
mod form;
mod path;
mod query;
mod raw_query;
mod request_parts;
mod tuple;

#[doc(inline)]
#[allow(deprecated)]
pub use self::{
    connect_info::ConnectInfo,
    content_length_limit::ContentLengthLimit,
    extension::Extension,
    extractor_middleware::extractor_middleware,
    form::Form,
    path::Path,
    query::Query,
    raw_query::RawQuery,
    request_parts::NestedUri,
    request_parts::{Body, BodyStream},
};
#[doc(no_inline)]
pub use crate::Json;

#[cfg(feature = "multipart")]
#[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
pub mod multipart;

#[cfg(feature = "multipart")]
#[cfg_attr(docsrs, doc(cfg(feature = "multipart")))]
#[doc(inline)]
pub use self::multipart::Multipart;

#[cfg(feature = "ws")]
#[cfg_attr(docsrs, doc(cfg(feature = "ws")))]
#[doc(inline)]
pub use self::ws::WebSocketUpgrade;

#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
mod typed_header;

#[cfg(feature = "headers")]
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
#[doc(inline)]
pub use self::typed_header::TypedHeader;

/// Types that can be created from requests.
///
/// See the [module docs](crate::extract) for more details.
///
/// # What is the `B` type parameter?
///
/// `FromRequest` is generic over the request body (the `B` in
/// [`http::Request<B>`]). This is to allow `FromRequest` to be usable will any
/// type of request body. This is necessary because some middleware change the
/// request body, for example to add timeouts.
///
/// If you're writing your own `FromRequest` that wont be used outside your
/// application, and not using any middleware that changes the request body, you
/// can most likely use `axum::body::Body`. Note this is also the default.
///
/// If you're writing a library, thats intended for others to use, its recommended
/// to keep the generic type parameter:
///
/// ```rust
/// use axum::{
///     async_trait,
///     extract::{FromRequest, RequestParts},
/// };
///
/// struct MyExtractor;
///
/// #[async_trait]
/// impl<B> FromRequest<B> for MyExtractor
/// where
///     B: Send, // required by `async_trait`
/// {
///     type Rejection = http::StatusCode;
///
///     async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
///         // ...
///         # unimplemented!()
///     }
/// }
/// ```
///
/// This ensures your extractor is as flexible as possible.
///
/// [`http::Request<B>`]: http::Request
#[async_trait]
pub trait FromRequest<B = crate::body::Body>: Sized {
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
pub struct RequestParts<B = crate::body::Body> {
    method: Method,
    uri: Uri,
    version: Version,
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
            method,
            uri,
            version,
            headers: Some(headers),
            extensions: Some(extensions),
            body: Some(body),
        }
    }

    // this method uses `Error` since we might make this method public one day and then
    // `Error` is more flexible.
    pub(crate) fn try_into_request(self) -> Result<Request<B>, Error> {
        let Self {
            method,
            uri,
            version,
            mut headers,
            mut extensions,
            mut body,
        } = self;

        let mut req = if let Some(body) = body.take() {
            Request::new(body)
        } else {
            return Err(Error::new(RequestAlreadyExtracted::BodyAlreadyExtracted(
                BodyAlreadyExtracted,
            )));
        };

        *req.method_mut() = method;
        *req.uri_mut() = uri;
        *req.version_mut() = version;

        if let Some(headers) = headers.take() {
            *req.headers_mut() = headers;
        } else {
            return Err(Error::new(
                RequestAlreadyExtracted::HeadersAlreadyExtracted(HeadersAlreadyExtracted),
            ));
        }

        if let Some(extensions) = extensions.take() {
            *req.extensions_mut() = extensions;
        } else {
            return Err(Error::new(
                RequestAlreadyExtracted::ExtensionsAlreadyExtracted(ExtensionsAlreadyExtracted),
            ));
        }

        Ok(req)
    }

    /// Gets a reference the request method.
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Gets a mutable reference to the request method.
    pub fn method_mut(&mut self) -> &mut Method {
        &mut self.method
    }

    /// Gets a reference the request URI.
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Gets a mutable reference to the request URI.
    pub fn uri_mut(&mut self) -> &mut Uri {
        &mut self.uri
    }

    /// Get the request HTTP version.
    pub fn version(&self) -> Version {
        self.version
    }

    /// Gets a mutable reference to the request HTTP version.
    pub fn version_mut(&mut self) -> &mut Version {
        &mut self.version
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

pub(crate) fn has_content_type<B>(
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

pub(crate) fn take_body<B>(req: &mut RequestParts<B>) -> Result<B, BodyAlreadyExtracted> {
    req.take_body().ok_or(BodyAlreadyExtracted)
}
