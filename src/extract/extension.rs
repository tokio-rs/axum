use super::{rejection::*, FromRequest, RequestParts};
use async_trait::async_trait;
use std::ops::Deref;

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
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If the extension is missing it will reject the request with a `500 Internal
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
            .ok_or_else(|| {
                MissingExtension::from_err(format!(
                    "Extension of type `{}` was not found. Perhaps you forgot to add it?",
                    std::any::type_name::<T>()
                ))
            })
            .map(|x| x.clone())?;

        Ok(Extension(value))
    }
}

impl<T> Deref for Extension<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
