use crate::{
    body::{box_body, Body, BoxBody},
    clone_box_service::CloneBoxService,
    BoxError,
};
use bytes::Bytes;
use http::{Request, Response};
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{util::Oneshot, ServiceBuilder, ServiceExt};
use tower_http::map_response_body::MapResponseBodyLayer;
use tower_layer::Layer;
use tower_service::Service;

/// [`Layer`] that applies the [`BoxRoute`] middleware.
///
/// Created with [`BoxRoute::layer`]. See [`BoxRoute`] for more details.
pub struct BoxRouteLayer<B = Body, E = Infallible>(PhantomData<fn() -> (B, E)>);

impl<B, E> fmt::Debug for BoxRouteLayer<B, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BoxRouteLayer").finish()
    }
}

impl<B, E> Clone for BoxRouteLayer<B, E> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<S, ReqBody, ResBody> Layer<S> for BoxRouteLayer<ReqBody, S::Error>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + Sync + 'static,
    S::Error: Into<BoxError> + Send,
    S::Future: Send,
    ReqBody: Send + 'static,
    ResBody: http_body::Body<Data = Bytes> + Send + Sync + 'static,
    ResBody::Error: Into<BoxError>,
{
    type Service = BoxRoute<ReqBody, S::Error>;

    fn layer(&self, inner: S) -> Self::Service {
        ServiceBuilder::new()
            .layer_fn(BoxRoute)
            .layer_fn(CloneBoxService::new)
            .layer(MapResponseBodyLayer::new(box_body))
            .service(inner)
    }
}

/// A boxed route trait object.
///
/// This makes it easier to name the types of routers to, for example, return
/// them from functions. Applied with `.layer(BoxRoute::<Body>::layer())`:
///
/// ```rust
/// use axum::{
///     body::Body,
///     handler::get,
///     routing::BoxRoute,
///     Router,
/// };
///
/// async fn first_handler() { /* ... */ }
///
/// async fn second_handler() { /* ... */ }
///
/// async fn third_handler() { /* ... */ }
///
/// fn app() -> Router<BoxRoute> {
///     Router::new()
///         .route("/", get(first_handler).post(second_handler))
///         .route("/foo", get(third_handler))
///         .layer(BoxRoute::<Body>::layer())
/// }
/// ```
///
/// Note that its important to specify the request body type with
/// `BoxRoute::<Body>::layer()` as this improves compile times.
pub struct BoxRoute<B = Body, E = Infallible>(CloneBoxService<Request<B>, Response<BoxBody>, E>);

impl<B, E> BoxRoute<B, E> {
    /// Get a [`BoxRouteLayer`] which is a [`Layer`] that applies the
    /// [`BoxRoute`] middleware.
    pub fn layer() -> BoxRouteLayer<B, E> {
        BoxRouteLayer(PhantomData)
    }
}

impl<B, E> Clone for BoxRoute<B, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<B, E> fmt::Debug for BoxRoute<B, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxRoute").finish()
    }
}

impl<B, E> Service<Request<B>> for BoxRoute<B, E>
where
    E: Into<BoxError>,
{
    type Response = Response<BoxBody>;
    type Error = E;
    type Future = BoxRouteFuture<B, E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        BoxRouteFuture {
            inner: self.0.clone().oneshot(req),
        }
    }
}

pin_project! {
    /// The response future for [`BoxRoute`].
    pub struct BoxRouteFuture<B, E>
    where
        E: Into<BoxError>,
    {
        #[pin]
        pub(super) inner: Oneshot<
            CloneBoxService<Request<B>, Response<BoxBody>, E>,
            Request<B>,
        >,
    }
}

impl<B, E> Future for BoxRouteFuture<B, E>
where
    E: Into<BoxError>,
{
    type Output = Result<Response<BoxBody>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

impl<B, E> fmt::Debug for BoxRouteFuture<B, E>
where
    E: Into<BoxError>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxRouteFuture").finish()
    }
}
