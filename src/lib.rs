#![allow(unused_imports, dead_code)]

/*

Improvements to make:

Support extracting headers, perhaps via `headers::Header`?

Tests

*/

use self::{
    body::{Body, BoxBody},
    extract::FromRequest,
    handler::{Handler, HandlerSvc},
    response::IntoResponse,
    routing::{EmptyRouter, RouteAt},
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{future, ready};
use http::{header, HeaderValue, Method, Request, Response, StatusCode};
use http_body::Body as _;
use pin_project::pin_project;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    convert::Infallible,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{BoxError, Layer, Service, ServiceExt};

pub mod body;
pub mod extract;
pub mod handler;
pub mod response;
pub mod routing;

mod error;

pub use self::error::Error;

pub fn app() -> App<EmptyRouter> {
    App {
        router: EmptyRouter(()),
    }
}

#[derive(Debug, Clone)]
pub struct App<R> {
    router: R,
}

impl<R> App<R> {
    pub fn at(self, route_spec: &str) -> RouteAt<R> {
        self.at_bytes(Bytes::copy_from_slice(route_spec.as_bytes()))
    }

    fn at_bytes(self, route_spec: Bytes) -> RouteAt<R> {
        RouteAt {
            app: self,
            route_spec,
        }
    }
}

pub struct IntoService<R> {
    app: App<R>,
    poll_ready_error: Option<Error>,
}

impl<R> Clone for IntoService<R>
where
    R: Clone,
{
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            poll_ready_error: None,
        }
    }
}

impl<R, B, T> Service<T> for IntoService<R>
where
    R: Service<T, Response = Response<B>>,
    R::Error: Into<Error>,
    B: Default,
{
    type Response = Response<B>;
    type Error = Error;
    type Future = HandleErrorFuture<R::Future, B>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if let Err(err) = ready!(self.app.router.poll_ready(cx)).map_err(Into::into) {
            self.poll_ready_error = Some(err);
        }

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: T) -> Self::Future {
        if let Some(poll_ready_error) = self.poll_ready_error.take() {
            match error::handle_error::<B>(poll_ready_error) {
                Ok(res) => {
                    return HandleErrorFuture(Kind::Response(Some(res)));
                }
                Err(err) => {
                    return HandleErrorFuture(Kind::Error(Some(err)));
                }
            }
        }
        HandleErrorFuture(Kind::Future(self.app.router.call(req)))
    }
}

#[pin_project]
pub struct HandleErrorFuture<F, B>(#[pin] Kind<F, B>);

#[pin_project(project = KindProj)]
enum Kind<F, B> {
    Response(Option<Response<B>>),
    Error(Option<Error>),
    Future(#[pin] F),
}

impl<F, B, E> Future for HandleErrorFuture<F, B>
where
    F: Future<Output = Result<Response<B>, E>>,
    E: Into<Error>,
    B: Default,
{
    type Output = Result<Response<B>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().0.project() {
            KindProj::Response(res) => Poll::Ready(Ok(res.take().unwrap())),
            KindProj::Error(err) => Poll::Ready(Err(err.take().unwrap())),
            KindProj::Future(fut) => match ready!(fut.poll(cx)) {
                Ok(res) => Poll::Ready(Ok(res)),
                Err(err) => Poll::Ready(error::handle_error(err.into())),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(warnings)]
    use super::*;
    use hyper::Server;
    use std::time::Duration;
    use std::{fmt, net::SocketAddr, sync::Arc};
    use tower::{
        layer::util::Identity, make::Shared, service_fn, timeout::TimeoutLayer, ServiceBuilder,
    };
    use tower_http::{
        add_extension::AddExtensionLayer,
        compression::CompressionLayer,
        trace::{Trace, TraceLayer},
    };

    #[tokio::test]
    async fn basic() {
        #[derive(Debug, Deserialize)]
        struct Pagination {
            page: usize,
            per_page: usize,
        }

        #[derive(Debug, Deserialize)]
        struct UsersCreate {
            username: String,
        }

        async fn root(_: Request<Body>) -> Result<Response<Body>, Error> {
            Ok(Response::new(Body::from("Hello, World!")))
        }

        async fn large_static_file(_: Request<Body>) -> Result<Response<Body>, Error> {
            Ok(Response::new(Body::empty()))
        }

        let app = app()
            // routes with functions
            .at("/")
            .get(root)
            // routes with closures
            .at("/users")
            .get(
                |_: Request<Body>, pagination: extract::Query<Pagination>| async {
                    let pagination = pagination.into_inner();
                    assert_eq!(pagination.page, 1);
                    assert_eq!(pagination.per_page, 30);
                    Ok::<_, Error>("users#index".to_string())
                },
            )
            .post(
                |_: Request<Body>,
                 payload: extract::Json<UsersCreate>,
                 _state: extract::Extension<Arc<State>>| async {
                    let payload = payload.into_inner();
                    assert_eq!(payload.username, "bob");
                    Ok::<_, Error>(response::Json(
                        serde_json::json!({ "username": payload.username }),
                    ))
                },
            )
            // routes with a service
            .at("/service")
            .get_service(service_fn(root))
            // routes with layers applied
            .at("/large-static-file")
            .get(
                large_static_file.layer(
                    ServiceBuilder::new()
                        .layer(TimeoutLayer::new(Duration::from_secs(30)))
                        .layer(CompressionLayer::new())
                        .into_inner(),
                ),
            )
            .into_service();

        // state shared by all routes, could hold db connection etc
        struct State {}

        let state = Arc::new(State {});

        // can add more middleware
        let mut app = ServiceBuilder::new()
            .layer(AddExtensionLayer::new(state))
            .layer(TraceLayer::new_for_http())
            .service(app);

        let res = app
            .ready()
            .await
            .unwrap()
            .call(
                Request::builder()
                    .method(Method::GET)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_to_string(res).await, "Hello, World!");

        let res = app
            .ready()
            .await
            .unwrap()
            .call(
                Request::builder()
                    .method(Method::GET)
                    .uri("/users?page=1&per_page=30")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_to_string(res).await, "users#index");

        let res = app
            .ready()
            .await
            .unwrap()
            .call(
                Request::builder()
                    .method(Method::GET)
                    .uri("/users")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body_to_string(res).await, "");

        let res = app
            .ready()
            .await
            .unwrap()
            .call(
                Request::builder()
                    .method(Method::POST)
                    .uri("/users")
                    .body(Body::from(r#"{ "username": "bob" }"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_to_string(res).await, r#"{"username":"bob"}"#);
    }

    async fn body_to_string<B>(res: Response<B>) -> String
    where
        B: http_body::Body,
        B::Error: fmt::Debug,
    {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[allow(dead_code)]
    // this should just compile
    async fn compatible_with_hyper_and_tower_http() {
        let app = app()
            .at("/")
            .get(|_: Request<Body>| async {
                Ok::<_, Error>(Response::new(Body::from("Hello, World!")))
            })
            .into_service();

        let app = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(CompressionLayer::new())
            .service(app);

        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let server = Server::bind(&addr).serve(Shared::new(app));
        server.await.unwrap();
    }
}
