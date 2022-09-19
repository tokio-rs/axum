use self::private::DefaultBodyLimitService;
use tower_layer::Layer;

/// Layer for configuring the default request body limit.
///
/// For security reasons, [`Bytes`] will, by default, not accept bodies larger than 2MB. This also
/// applies to extractors that uses [`Bytes`] internally such as `String`, [`Json`], and [`Form`].
///
/// This middleware provides ways to configure that.
///
/// Note that if an extractor consumes the body directly with [`Body::data`], or similar, the
/// default limit is _not_ applied.
///
/// [`Body::data`]: http_body::Body::data
/// [`Bytes`]: bytes::Bytes
/// [`Json`]: https://docs.rs/axum/0.6.0-rc.2/axum/struct.Json.html
/// [`Form`]: https://docs.rs/axum/0.6.0-rc.2/axum/struct.Form.html
#[derive(Debug, Clone)]
pub struct DefaultBodyLimit {
    kind: DefaultBodyLimitKind,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DefaultBodyLimitKind {
    Disable,
}

impl DefaultBodyLimit {
    /// Disable the default request body limit.
    ///
    /// This must be used to receive bodies larger than the default limit of 2MB using [`Bytes`] or
    /// an extractor built on it such as `String`, [`Json`], [`Form`].
    ///
    /// Note that if you're accepting data from untrusted remotes it is recommend to add your own
    /// limit such as [`tower_http::limit`].
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     body::{Bytes, Body},
    ///     extract::DefaultBodyLimit,
    /// };
    /// use tower_http::limit::RequestBodyLimitLayer;
    /// use http_body::Limited;
    ///
    /// let app: Router<_, Limited<Body>> = Router::new()
    ///     .route("/", get(|body: Bytes| async {}))
    ///     // Disable the default limit
    ///     .layer(DefaultBodyLimit::disable())
    ///     // Set a different limit
    ///     .layer(RequestBodyLimitLayer::new(10 * 1000 * 1000));
    /// ```
    ///
    /// [`tower_http::limit`]: https://docs.rs/tower-http/0.3.4/tower_http/limit/index.html
    /// [`Bytes`]: bytes::Bytes
    /// [`Json`]: https://docs.rs/axum/0.6.0-rc.2/axum/struct.Json.html
    /// [`Form`]: https://docs.rs/axum/0.6.0-rc.2/axum/struct.Form.html
    pub fn disable() -> Self {
        Self {
            kind: DefaultBodyLimitKind::Disable,
        }
    }
}

impl<S> Layer<S> for DefaultBodyLimit {
    type Service = DefaultBodyLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DefaultBodyLimitService {
            inner,
            kind: self.kind,
        }
    }
}

mod private {
    use super::DefaultBodyLimitKind;
    use http::Request;
    use std::task::Context;
    use tower_service::Service;

    #[derive(Debug, Clone, Copy)]
    pub struct DefaultBodyLimitService<S> {
        pub(super) inner: S,
        pub(super) kind: DefaultBodyLimitKind,
    }

    impl<B, S> Service<Request<B>> for DefaultBodyLimitService<S>
    where
        S: Service<Request<B>>,
    {
        type Response = S::Response;
        type Error = S::Error;
        type Future = S::Future;

        #[inline]
        fn poll_ready(&mut self, cx: &mut Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
            self.inner.poll_ready(cx)
        }

        #[inline]
        fn call(&mut self, mut req: Request<B>) -> Self::Future {
            req.extensions_mut().insert(self.kind);
            self.inner.call(req)
        }
    }
}
