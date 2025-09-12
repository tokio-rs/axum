//! Additional middleware utilities.

use crate::either::Either;
use axum::middleware::ResponseAxumBodyLayer;
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
/// `axum_extra::middleware::option_layer` makes sure that the output `Body` is [`axum::body::Body`].
///
/// [`Layer`]: tower_layer::Layer
pub fn option_layer<L>(layer: Option<L>) -> Either<(ResponseAxumBodyLayer, L), Identity> {
    layer
        .map(|layer| Either::E1((ResponseAxumBodyLayer, layer)))
        .unwrap_or_else(|| Either::E2(Identity::new()))
}

#[cfg(test)]
mod tests {
    use std::{
        convert::Infallible,
        pin::Pin,
        task::{Context, Poll},
    };

    use axum::{body::Body as AxumBody, Router};
    use bytes::Bytes;
    use http_body::Body as HttpBody;
    use tower_http::map_response_body::MapResponseBodyLayer;

    use super::option_layer;

    #[test]
    fn remap_response_body() {
        struct BodyWrapper;

        impl BodyWrapper {
            fn new(_: AxumBody) -> Self {
                Self
            }
        }

        impl HttpBody for BodyWrapper {
            type Data = Bytes;
            type Error = Infallible;
            fn poll_frame(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
                unimplemented!()
            }
            fn is_end_stream(&self) -> bool {
                unimplemented!()
            }
            fn size_hint(&self) -> http_body::SizeHint {
                unimplemented!()
            }
        }
        let _app: Router = Router::new().layer(option_layer(Some(MapResponseBodyLayer::new(
            BodyWrapper::new,
        ))));
    }
}
