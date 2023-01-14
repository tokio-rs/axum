//! Additional middleware utilities.

use crate::either::Either;
use tower_layer::Identity;

/// Convert an `Option<Layer>` into a [`Layer`].
///
/// If the layer is a `Some` it'll be applied, otherwise not.
///
/// # Example
///
/// ```
/// use axum_extra::middleware::option_layer;
/// use axum::{Router, routing::get};
/// use std::time::Duration;
/// use tower_http::timeout::TimeoutLayer;
///
/// # let option_timeout = Some(Duration::new(10, 0));
/// let timeout_layer = option_timeout.map(TimeoutLayer::new);
///
/// let app = Router::new()
///     .route("/", get(|| async {}))
///     .layer(option_layer(timeout_layer));
/// # let _: Router = app;
/// ```
///
/// # Difference between this and [`tower::util::option_layer`]
///
/// [`tower::util::option_layer`] always changes the error type to [`BoxError`] which requires
/// using [`HandleErrorLayer`] when used with axum, even if the layer you're applying uses
/// [`Infallible`].
///
/// `axum_extra::middleware::option_layer` on the other hand doesn't change the error type so can
/// be applied directly.
///
/// [`Layer`]: tower_layer::Layer
/// [`BoxError`]: tower::BoxError
/// [`HandleErrorLayer`]: axum::error_handling::HandleErrorLayer
/// [`Infallible`]: std::convert::Infallible
pub fn option_layer<L>(layer: Option<L>) -> Either<L, Identity> {
    layer
        .map(Either::E1)
        .unwrap_or_else(|| Either::E2(Identity::new()))
}
