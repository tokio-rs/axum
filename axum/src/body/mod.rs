//! HTTP body utilities.

#[doc(no_inline)]
pub use http_body::Body as HttpBody;

#[doc(no_inline)]
pub use bytes::Bytes;

#[doc(inline)]
pub use axum_core::body::Body;

use http_body_util::BodyExt;

/// Converts [`Body`] into [`Bytes`].
/// ```rust
/// # use axum::body::to_bytes;
/// async fn foo() {
///     let body = Body::from(vec![1, 2, 3]);
///     assert_eq!(&to_bytes(body).await.unwrap()[..], &[1, 2, 3]);
/// }
/// ```
pub async fn to_bytes(body: Body) -> Result<Bytes, axum_core::Error> {
    body.collect().await.map(|col| col.to_bytes())
}
