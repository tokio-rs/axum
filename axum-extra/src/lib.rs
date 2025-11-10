//! Extra utilities for [`axum`].
//!
//! # Feature flags
//!
//! axum-extra uses a set of [feature flags] to reduce the amount of compiled and
//! optional dependencies.
//!
//! The following optional features are available:
//!
//! Name | Description | Default?
//! ---|---|---
//! `async-read-body` | Enables the [`AsyncReadBody`](crate::body::AsyncReadBody) body |
//! `attachment` | Enables the [`Attachment`](crate::response::Attachment) response |
//! `cached` | Enables the [`Cached`](crate::extract::Cached) extractor |
//! `cookie` | Enables the [`CookieJar`](crate::extract::CookieJar) extractor |
//! `cookie-private` | Enables the [`PrivateCookieJar`](crate::extract::PrivateCookieJar) extractor |
//! `cookie-signed` | Enables the [`SignedCookieJar`](crate::extract::SignedCookieJar) extractor |
//! `cookie-key-expansion` | Enables the [`Key::derive_from`](crate::extract::cookie::Key::derive_from) method |
//! `erased-json` | Enables the [`ErasedJson`](crate::response::ErasedJson) response |
//! `error-response` | Enables the [`InternalServerError`](crate::response::InternalServerError) response |
//! `form` | Enables the [`Form`](crate::extract::Form) extractor |
//! `handler` | Enables the [handler] utilities |
//! `json-deserializer` | Enables the [`JsonDeserializer`](crate::extract::JsonDeserializer) extractor |
//! `json-lines` | Enables the [`JsonLines`](crate::extract::JsonLines) extractor and response |
//! `middleware` | Enables the [middleware] utilities |
//! `multipart` | Enables the [`Multipart`](crate::extract::Multipart) extractor |
//! `optional-path` | Enables the [`OptionalPath`](crate::extract::OptionalPath) extractor |
//! `protobuf` | Enables the [`Protobuf`](crate::protobuf::Protobuf) extractor and response |
//! `query` | Enables the [`Query`](crate::extract::Query) extractor |
//! `routing` | Enables the [routing] utilities |
//! `tracing` | Log rejections from built-in extractors | <span role="img" aria-label="Default feature">✔</span>
//! `typed-routing` | Enables the [`TypedPath`](crate::routing::TypedPath) routing utilities and the `routing` feature. |
//! `typed-header` | Enables the [`TypedHeader`] extractor and response |
//! `file-stream` | Enables the [`FileStream`](crate::response::FileStream) response |
//! `with-rejection` | Enables the [`WithRejection`](crate::extract::WithRejection) extractor |
//!
//! [`axum`]: https://crates.io/crates/axum

#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(not(test), warn(clippy::print_stdout, clippy::dbg_macro))]

#[allow(unused_extern_crates)]
extern crate self as axum_extra;

pub mod body;
pub mod either;
pub mod extract;
pub mod response;

#[cfg(feature = "routing")]
pub mod routing;

#[cfg(feature = "middleware")]
pub mod middleware;

#[cfg(feature = "handler")]
pub mod handler;

#[cfg(feature = "json-lines")]
pub mod json_lines;

#[cfg(feature = "typed-header")]
pub mod typed_header;

#[cfg(feature = "typed-header")]
#[doc(no_inline)]
pub use headers;

#[cfg(feature = "typed-header")]
#[doc(inline)]
pub use typed_header::TypedHeader;

#[cfg(feature = "protobuf")]
pub mod protobuf;

/// _not_ public API
#[cfg(feature = "typed-routing")]
#[doc(hidden)]
pub mod __private {
    use percent_encoding::{AsciiSet, CONTROLS};

    pub use percent_encoding::utf8_percent_encode;

    // from https://github.com/servo/rust-url/blob/master/url/src/parser.rs
    const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
    const PATH: &AsciiSet = &FRAGMENT.add(b'#').add(b'?').add(b'{').add(b'}');
    pub const PATH_SEGMENT: &AsciiSet = &PATH.add(b'/').add(b'%');
}

#[cfg(test)]
use axum_macros::__private_axum_test as test;

#[cfg(test)]
pub(crate) use axum::test_helpers;
