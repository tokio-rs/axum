use super::{Handler, IntoService};
use crate::{extract::connect_info::IntoMakeServiceWithConnectInfo, routing::IntoMakeService};
use http::Request;
use std::task::{Context, Poll};
use tower_service::Service;

/// A [`Handler`] with state provided.
///
/// Implements [`Service`].
///
/// Created with [`Handler::with_state`].
pub struct WithState<H, T, S, B> {
    pub(super) service: IntoService<H, T, S, B>,
}

impl<H, T, S, B> WithState<H, T, S, B> {
    /// Convert the handler into a [`MakeService`].
    ///
    /// This allows you to serve a single handler if you don't need any routing:
    ///
    /// ```rust
    /// use axum::{
    ///     Server, handler::Handler, http::{Uri, Method}, response::IntoResponse,
    /// };
    /// use std::net::SocketAddr;
    ///
    /// async fn handler(method: Method, uri: Uri, body: String) -> String {
    ///     format!("received `{} {}` with body `{:?}`", method, uri, body)
    /// }
    ///
    /// # async {
    /// Server::bind(&SocketAddr::from(([127, 0, 0, 1], 3000)))
    ///     .serve(handler.with_state(()).into_make_service())
    ///     .await?;
    /// # Ok::<_, hyper::Error>(())
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    pub fn into_make_service(self) -> IntoMakeService<IntoService<H, T, S, B>> {
        IntoMakeService::new(self.service)
    }

    /// Convert the handler into a [`MakeService`] which stores information
    /// about the incoming connection.
    ///
    /// See [`Router::into_make_service_with_connect_info`] for more details.
    ///
    /// ```rust
    /// use axum::{
    ///     Server,
    ///     handler::Handler,
    ///     response::IntoResponse,
    ///     extract::ConnectInfo,
    /// };
    /// use std::net::SocketAddr;
    ///
    /// async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
    ///     format!("Hello {}", addr)
    /// }
    ///
    /// # async {
    /// Server::bind(&SocketAddr::from(([127, 0, 0, 1], 3000)))
    ///     .serve(handler.with_state(()).into_make_service_with_connect_info::<SocketAddr>())
    ///     .await?;
    /// # Ok::<_, hyper::Error>(())
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    /// [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info
    pub fn into_make_service_with_connect_info<C>(
        self,
    ) -> IntoMakeServiceWithConnectInfo<IntoService<H, T, S, B>, C> {
        IntoMakeServiceWithConnectInfo::new(self.service)
    }
}

impl<H, T, S, B> Service<Request<B>> for WithState<H, T, S, B>
where
    H: Handler<T, S, B> + Clone + Send + 'static,
    B: Send + 'static,
    S: Clone,
{
    type Response = <IntoService<H, T, S, B> as Service<Request<B>>>::Response;
    type Error = <IntoService<H, T, S, B> as Service<Request<B>>>::Error;
    type Future = <IntoService<H, T, S, B> as Service<Request<B>>>::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        self.service.call(req)
    }
}

impl<H, T, S, B> std::fmt::Debug for WithState<H, T, S, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WithState")
            .field("service", &self.service)
            .finish()
    }
}

impl<H, T, S, B> Clone for WithState<H, T, S, B>
where
    H: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
        }
    }
}
