use crate::{
    body::{self, Bytes, HttpBody},
    response::{IntoResponse, Response},
    BoxError,
};
use axum_core::extract::{FromRequest, RequestParts};
use futures_util::future::BoxFuture;
use http::Request;
use std::{
    any::type_name,
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{util::BoxCloneService, ServiceBuilder};
use tower_http::ServiceBuilderExt;
use tower_layer::Layer;
use tower_service::Service;

mod generated;

/// Create a middleware from an async function.
///
/// `from_fn` requires the function given to
///
/// 1. Be an `async fn`.
/// 2. Take one or more [extractors] as the first arguments.
/// 3. Take [`Next<B>`](Next) as the final argument.
/// 4. Return something that implements [`IntoResponse`].
///
/// # Example
///
/// ```rust
/// use axum::{
///     Router,
///     http::{Request, StatusCode},
///     routing::get,
///     response::{IntoResponse, Response},
///     middleware::{self, Next},
/// };
///
/// async fn auth<B>(req: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
///     let auth_header = req.headers()
///         .get(http::header::AUTHORIZATION)
///         .and_then(|header| header.to_str().ok());
///
///     match auth_header {
///         Some(auth_header) if token_is_valid(auth_header) => {
///             Ok(next.run(req).await)
///         }
///         _ => Err(StatusCode::UNAUTHORIZED),
///     }
/// }
///
/// fn token_is_valid(token: &str) -> bool {
///     // ...
///     # false
/// }
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .route_layer(middleware::from_fn(auth));
/// # let app: Router = app;
/// ```
///
/// # Running extractors
///
/// ```rust
/// use axum::{
///     Router,
///     extract::{TypedHeader, Query},
///     headers::authorization::{Authorization, Bearer},
///     http::Request,
///     middleware::{self, Next},
///     response::Response,
///     routing::get,
/// };
/// use std::collections::HashMap;
///
/// async fn my_middleware<B>(
///     TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
///     Query(query_params): Query<HashMap<String, String>>,
///     req: Request<B>,
///     next: Next<B>,
/// ) -> Response {
///     // do something with `auth` and `query_params`...
///
///     next.run(req).await
/// }
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .route_layer(middleware::from_fn(my_middleware));
/// # let app: Router = app;
/// ```
///
/// # Passing state
///
/// State can be passed to the function like so:
///
/// ```rust
/// use axum::{
///     Router,
///     http::{Request, StatusCode},
///     routing::get,
///     response::{IntoResponse, Response},
///     middleware::{self, Next}
/// };
///
/// #[derive(Clone)]
/// struct State { /* ... */ }
///
/// async fn my_middleware<B>(
///     req: Request<B>,
///     next: Next<B>,
///     state: State,
/// ) -> Response {
///     // ...
///     # ().into_response()
/// }
///
/// let state = State { /* ... */ };
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .route_layer(middleware::from_fn(move |req, next| {
///         my_middleware(req, next, state.clone())
///     }));
/// # let app: Router = app;
/// ```
///
/// Or via extensions:
///
/// ```rust
/// use axum::{
///     Router,
///     extract::Extension,
///     http::{Request, StatusCode},
///     routing::get,
///     response::{IntoResponse, Response},
///     middleware::{self, Next},
/// };
/// use tower::ServiceBuilder;
///
/// #[derive(Clone)]
/// struct State { /* ... */ }
///
/// async fn my_middleware<B>(
///     Extension(state): Extension<State>,
///     req: Request<B>,
///     next: Next<B>,
/// ) -> Response {
///     // ...
///     # ().into_response()
/// }
///
/// let state = State { /* ... */ };
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .layer(
///         ServiceBuilder::new()
///             .layer(Extension(state))
///             .layer(middleware::from_fn(my_middleware)),
///     );
/// # let app: Router = app;
/// ```
///
/// [extractors]: crate::extract::FromRequest
pub fn from_fn<F, T>(f: F) -> FromFnLayer<F, T> {
    FromFnLayer {
        f,
        _extractor: PhantomData,
    }
}

/// A [`tower::Layer`] from an async function.
///
/// [`tower::Layer`] is used to apply middleware to [`Router`](crate::Router)'s.
///
/// Created with [`from_fn`]. See that function for more details.
pub struct FromFnLayer<F, T> {
    f: F,
    _extractor: PhantomData<fn() -> T>,
}

impl<F, T> Clone for FromFnLayer<F, T>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            _extractor: self._extractor,
        }
    }
}

impl<F, T> Copy for FromFnLayer<F, T> where F: Copy {}

impl<S, F, T> Layer<S> for FromFnLayer<F, T>
where
    F: Clone,
{
    type Service = FromFn<F, S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        FromFn {
            f: self.f.clone(),
            inner,
            _extractor: PhantomData,
        }
    }
}

impl<F, T> fmt::Debug for FromFnLayer<F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromFnLayer")
            // Write out the type name, without quoting it as `&type_name::<F>()` would
            .field("f", &format_args!("{}", type_name::<F>()))
            .finish()
    }
}

/// A middleware created from an async function.
///
/// Created with [`from_fn`]. See that function for more details.
pub struct FromFn<F, S, T> {
    f: F,
    inner: S,
    _extractor: PhantomData<fn() -> T>,
}

impl<F, S, T> Clone for FromFn<F, S, T>
where
    F: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            inner: self.inner.clone(),
            _extractor: self._extractor,
        }
    }
}

impl<F, S, T> Copy for FromFn<F, S, T>
where
    F: Copy,
    S: Copy,
{
}

impl<F, S, T> fmt::Debug for FromFn<F, S, T>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromFnLayer")
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("inner", &self.inner)
            .finish()
    }
}

/// The remainder of a middleware stack, including the handler.
pub struct Next<ReqBody> {
    inner: BoxCloneService<Request<ReqBody>, Response, Infallible>,
}

impl<ReqBody> Next<ReqBody> {
    /// Execute the remaining middleware stack.
    pub async fn run(mut self, req: Request<ReqBody>) -> Response {
        match self.inner.call(req).await {
            Ok(res) => res,
            Err(err) => match err {},
        }
    }
}

impl<ReqBody> fmt::Debug for Next<ReqBody> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromFnLayer")
            .field("inner", &self.inner)
            .finish()
    }
}

/// Response future for [`FromFn`].
pub struct ResponseFuture {
    inner: BoxFuture<'static, Response>,
}

impl Future for ResponseFuture {
    type Output = Result<Response, Infallible>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.as_mut().poll(cx).map(Ok)
    }
}

impl fmt::Debug for ResponseFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseFuture").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{body::Empty, routing::get, Router};
    use async_trait::async_trait;
    use axum_core::extract::{FromRequest, Mut, Once, RequestParts};
    use http::{HeaderMap, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn basic() {
        async fn insert_header<B>(mut req: Request<B>, next: Next<B>) -> impl IntoResponse {
            req.headers_mut()
                .insert("x-axum-test", "ok".parse().unwrap());

            next.run(req).await
        }

        async fn handle(headers: HeaderMap) -> String {
            (&headers["x-axum-test"]).to_str().unwrap().to_owned()
        }

        let app = Router::new()
            .route("/", get(handle))
            .layer(from_fn(insert_header));

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(body::boxed(Empty::new()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(res).await.unwrap();
        assert_eq!(&body[..], b"ok");
    }

    // this just needs to compile
    #[allow(dead_code)]
    fn running_extractor() {
        struct MyExtractorAny;

        #[async_trait]
        impl<R, B> FromRequest<R, B> for MyExtractorAny
        where
            B: Send,
        {
            type Rejection = ();

            async fn from_request(_req: &mut RequestParts<R, B>) -> Result<Self, Self::Rejection> {
                Ok(Self)
            }
        }

        struct MyExtractorOnce;

        #[async_trait]
        impl<B> FromRequest<Once, B> for MyExtractorOnce
        where
            B: Send,
        {
            type Rejection = ();

            async fn from_request(
                _req: &mut RequestParts<Once, B>,
            ) -> Result<Self, Self::Rejection> {
                Ok(Self)
            }
        }

        async fn run_mut<B>(req: Request<B>, next: Next<B>) -> Response
        where
            B: Send,
        {
            let mut req = RequestParts::<Mut, _>::new(req);
            let _: MyExtractorAny = req.extract::<MyExtractorAny>().await.unwrap();
            let req = req.into_request();
            next.run(req).await
        }

        async fn run_once<B>(req: Request<B>, next: Next<B>) -> Response
        where
            B: Send,
        {
            let mut req = RequestParts::<Once, _>::new(req);
            let _: MyExtractorAny = req.extract::<MyExtractorAny>().await.unwrap();
            let req = req.try_into_request().unwrap();
            next.run(req).await
        }

        let _: Router = Router::new()
            .layer(from_fn(run_mut))
            .layer(from_fn(run_once));
    }
}
