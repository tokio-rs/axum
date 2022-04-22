//! Convert an extractor into a middleware.
//!
//! See [`extractor_middleware`] for more details.

use crate::middleware::{from_extractor, FromExtractorLayer};

/// Convert an extractor into a middleware.
///
/// Deprecated, please use [`crate::middleware::from_extractor`] instead
///
#[deprecated(note = "Please use `axum::middleware::from_extractor` instead")]
pub fn extractor_middleware<E>() -> FromExtractorLayer<E> {
    from_extractor()
}
