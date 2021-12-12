// this is vendored from tower-http to reduce public dependencies

use http::Request;
use std::task::{Context, Poll};
use tower_layer::Layer;
use tower_service::Service;

/// [`Layer`] for adding some shareable value to [request extensions].
///
/// See [Sharing state with handlers](index.html#sharing-state-with-handlers)
/// for more details.
///
/// [request extensions]: https://docs.rs/http/latest/http/struct.Extensions.html
#[derive(Clone, Copy, Debug)]
pub struct AddExtensionLayer<T> {
    value: T,
}

impl<T> AddExtensionLayer<T> {
    /// Create a new [`AddExtensionLayer`].
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<S, T> Layer<S> for AddExtensionLayer<T>
where
    T: Clone,
{
    type Service = AddExtension<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        AddExtension {
            inner,
            value: self.value.clone(),
        }
    }
}

/// Middleware for adding some shareable value to [request extensions].
///
/// See [Sharing state with handlers](index.html#sharing-state-with-handlers)
/// for more details.
///
/// [request extensions]: https://docs.rs/http/latest/http/struct.Extensions.html
#[derive(Clone, Copy, Debug)]
pub struct AddExtension<S, T> {
    inner: S,
    value: T,
}

impl<S, T> AddExtension<S, T> {
    /// Create a new [`AddExtensionLayer`].
    pub fn layer(value: T) -> AddExtensionLayer<T> {
        AddExtensionLayer::new(value)
    }
}

impl<ResBody, S, T> Service<Request<ResBody>> for AddExtension<S, T>
where
    S: Service<Request<ResBody>>,
    T: Clone + Send + Sync + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ResBody>) -> Self::Future {
        req.extensions_mut().insert(self.value.clone());
        self.inner.call(req)
    }
}
