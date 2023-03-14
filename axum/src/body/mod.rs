//! HTTP body utilities.

#[doc(no_inline)]
pub use http_body::{Body as HttpBody, Empty, Full};

#[doc(no_inline)]
pub use bytes::Bytes;

#[doc(inline)]
pub use axum_core::body::{boxed, Body, BoxBody};
