# Applying middleware

axum is designed to take full advantage of the tower and tower-http
ecosystem of middleware.

If you're new to tower we recommend you read its [guides][tower-guides] for
a general introduction to tower and its concepts.

## To individual handlers

A middleware can be applied to a single handler like so:

```rust,no_run
use axum::{
    handler::Handler,
    routing::get,
    Router,
};
use tower::limit::ConcurrencyLimitLayer;

let app = Router::new()
    .route(
        "/",
        get(handler.layer(ConcurrencyLimitLayer::new(100))),
    );

async fn handler() {}
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

## To groups of routes

Middleware can also be applied to a group of routes like so:

```rust,no_run
use axum::{
    routing::{get, post},
    Router,
};
use tower::limit::ConcurrencyLimitLayer;

async fn handler() {}

let app = Router::new()
    .route("/", get(handler))
    .route("/foo", post(handler))
    .layer(ConcurrencyLimitLayer::new(100));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Note that [`Router::layer`] applies the middleware to all previously added
routes, of that particular `Router`. If you need multiple groups of routes
with different middleware build them separately and combine them with
[`Router::merge`]:

```rust,no_run
use axum::{
    routing::{get, post},
    Router,
};
use tower::limit::ConcurrencyLimitLayer;
# type MyAuthLayer = tower::layer::util::Identity;

async fn handler() {}

let foo = Router::new()
    .route("/", get(handler))
    .route("/foo", post(handler))
    .layer(ConcurrencyLimitLayer::new(100));

let bar = Router::new()
    .route("/requires-auth", get(handler))
    .layer(MyAuthLayer::new());

let app = foo.merge(bar);
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

## Applying multiple middleware

[`tower::ServiceBuilder`] can be used to combine multiple middleware:

```rust,no_run
use axum::{
    body::Body,
    routing::get,
    http::{Request, StatusCode},
    error_handling::HandleErrorLayer,
    response::IntoResponse,
    Router, BoxError,
};
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use std::{borrow::Cow, time::Duration};

let middleware_stack = ServiceBuilder::new()
    // Handle errors from middleware
    //
    // This middleware most be added above any fallible
    // ones if you're using `ServiceBuilder`, due to how ordering works
    .layer(HandleErrorLayer::new(handle_error))
    // Return an error after 30 seconds
    .timeout(Duration::from_secs(30))
    // Shed load if we're receiving too many requests
    .load_shed()
    // Process at most 100 requests concurrently
    .concurrency_limit(100)
    // Compress response bodies
    .layer(CompressionLayer::new());

let app = Router::new()
    .route("/", get(|_: Request<Body>| async { /* ... */ }))
    .layer(middleware_stack);

fn handle_error(error: BoxError) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Something went wrong: {}", error),
    )
}
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

See [Error handling](#error-handling) for more details on general error handling in axum.

## Commonly used middleware

[`tower::util`] and [`tower_http`] have a large collection of middleware that are compatible
with axum. Some commonly used are:

```rust,no_run
use axum::{
    body::{Body, BoxBody},
    routing::get,
    http::{Request, Response},
    error_handling::HandleErrorLayer,
    Router,
};
use tower::{
    filter::AsyncFilterLayer,
    util::AndThenLayer,
    ServiceBuilder,
};
use std::convert::Infallible;
use tower_http::trace::TraceLayer;
#
# fn handle_error<T>(error: T) -> axum::http::StatusCode {
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

async fn map_response(res: Response<BoxBody>) -> Result<Response<BoxBody>, Infallible> {
    Ok(res)
}

let app = Router::new()
    .route("/", get(|| async { /* ... */ }))
    .layer(middleware_stack);
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Additionally axum provides [`extract::extractor_middleware()`] for converting any extractor into
a middleware. Among other things, this can be useful for doing authorization. See
[`extract::extractor_middleware()`] for more details.

See [Error handling](#error-handling) for more details on general error handling in axum.

## Writing your own middleware

You can also write you own middleware by implementing [`tower::Service`]:

```
use axum::{
    body::{Body, BoxBody},
    routing::get,
    http::{Request, Response},
    Router,
};
use futures::future::BoxFuture;
use tower::{Service, layer::layer_fn};
use std::task::{Context, Poll};

#[derive(Clone)]
struct MyMiddleware<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for MyMiddleware<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        println!("`MyMiddleware` called!");

        // best practice is to clone the inner service like this
        // see https://github.com/tower-rs/tower/issues/547 for details
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            let res: Response<ResBody> = inner.call(req).await?;

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
