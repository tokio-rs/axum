axum is designed to take full advantage of the [`tower`] and [`tower-http`]
ecosystem of middleware.

If you're new to tower we recommend you read its [guides][tower-guides] for
a general introduction to tower and its concepts.

axum supports adding middleware to both individual handlers and entire routers.
For more details on that see

- [Individual handlers](crate::handler::Handler::layer)
- [Routers](crate::routing::Router::layer)

## Applying multiple middleware

It's recommended to use [`tower::ServiceBuilder`] to apply multiple middleware at
once, instead of calling [`Router::layer`] repeatedly:

```rust
use axum::{
    routing::get,
    AddExtensionLayer,
    Router,
};
use tower_http::{trace::TraceLayer};
use tower::{ServiceBuilder, limit::ConcurrencyLimitLayer};

async fn handler() {}

#[derive(Clone)]
struct State {}

let app = Router::new()
    .route("/", get(handler))
    .layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(ConcurrencyLimitLayer::new(64))
            .layer(AddExtensionLayer::new(State {}))
    );
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

## Middleware and errors

If you're applying middleware that produces errors you have to handle the errors
so they're converted into responses. You can learn more about doing that
[here](crate::error_handling).

## Commonly used middleware

[`tower`] and [`tower_http`] have a large collection of middleware that are
compatible with axum. Some commonly used middleware are:

```rust,no_run
use axum::{
	response::Response,
    Router,
    body::{Body, BoxBody},
    error_handling::HandleErrorLayer,
    http::Request,
    routing::get,
};
use tower::{
    filter::AsyncFilterLayer,
    util::AndThenLayer,
    ServiceBuilder,
};
use std::convert::Infallible;
use tower_http::trace::TraceLayer;
#
# async fn handle_error<T>(error: T) -> axum::http::StatusCode {
#     axum::http::StatusCode::INTERNAL_SERVER_ERROR
# }

let middleware_stack = ServiceBuilder::new()
    // Handle errors from middleware
    //
    // This middleware most be added above any fallible
    // ones if you're using `ServiceBuilder`, due to how ordering works
    .layer(HandleErrorLayer::new(handle_error))
    // `TraceLayer` adds high level tracing and logging
    .layer(TraceLayer::new_for_http())
    // `AsyncFilterLayer` lets you asynchronously transform the request
    .layer(AsyncFilterLayer::new(map_request))
    // `AndThenLayer` lets you asynchronously transform the response
    .layer(AndThenLayer::new(map_response));

async fn map_request(req: Request<Body>) -> Result<Request<Body>, Infallible> {
    Ok(req)
}

async fn map_response(res: Response) -> Result<Response, Infallible> {
    Ok(res)
}

let app = Router::new()
    .route("/", get(|| async { /* ... */ }))
    .layer(middleware_stack);
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Additionally axum provides [`extract::extractor_middleware()`] for converting
any extractor into a middleware. See [`extract::extractor_middleware()`] for
more details.

## Writing your own middleware with `axum_extra::middleware::from_fn`

The easiest way to write a custom middleware is using
[`axum_extra::middleware::from_fn`]. See that function for more details.

[`axum_extra::middleware::from_fn`]: https://docs.rs/axum-extra/0.1/axum_extra/middleware/middleware_fn/fn.from_fn.html

## Writing your own middleware with `tower::Service`

For maximum control (and a more low level API) you can write you own middleware
by implementing [`tower::Service`]:

```rust
use axum::{
    response::Response,
    Router,
    body::{Body, BoxBody},
    http::Request,
    routing::get,
};
use futures::future::BoxFuture;
use tower::{Service, layer::layer_fn};
use std::task::{Context, Poll};

#[derive(Clone)]
struct MyMiddleware<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for MyMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        println!("`MyMiddleware` called!");

        // best practice is to clone the inner service like this
        // see https://github.com/tower-rs/tower/issues/547 for details
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            let res: Response = inner.call(req).await?;

            println!("`MyMiddleware` received the response");

            Ok(res)
        })
    }
}

let app = Router::new()
    .route("/", get(|| async { /* ... */ }))
    .layer(layer_fn(|inner| MyMiddleware { inner }));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```
