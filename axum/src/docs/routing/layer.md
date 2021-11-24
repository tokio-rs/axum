Apply a [`tower::Layer`] to the router.

All requests to the router will be processed by the layer's
corresponding middleware.

This can be used to add additional processing to a request for a group
of routes.

Note this differs from [`handler::Layered`](crate::handler::Layered)
which adds a middleware to a single handler.

# Example

Adding the [`tower::limit::ConcurrencyLimit`] middleware to a group of
routes can be done like so:

```rust
use axum::{
    routing::get,
    Router,
};
use tower::limit::{ConcurrencyLimitLayer, ConcurrencyLimit};

async fn first_handler() {}

async fn second_handler() {}

async fn third_handler() {}

// All requests to `first_handler` and `second_handler` will be sent through
// `ConcurrencyLimit`
let app = Router::new().route("/", get(first_handler))
    .route("/foo", get(second_handler))
    .layer(ConcurrencyLimitLayer::new(64))
    // Request to `GET /bar` will go directly to `third_handler` and
    // wont be sent through `ConcurrencyLimit`
    .route("/bar", get(third_handler));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

This is commonly used to add middleware such as tracing/logging to your
entire app:

```rust
use axum::{
    routing::get,
    Router,
};
use tower_http::trace::TraceLayer;

async fn first_handler() {}

async fn second_handler() {}

async fn third_handler() {}

let app = Router::new()
    .route("/", get(first_handler))
    .route("/foo", get(second_handler))
    .route("/bar", get(third_handler))
    .layer(TraceLayer::new_for_http());
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

# Applying multiple middleware

Its recommended to use [`tower::ServiceBuilder`] to apply multiple middleware at
once, instead of calling `layer` repeatedly:

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

# Error handling

axum's error handling model requires handlers to always return a response.
However middleware is one possible way to introduce errors into an application.
If hyper receives an error the connection will be closed without sending a
response. Thus axum requires those errors to be handled gracefully:

```rust
use axum::{
    routing::get,
    error_handling::HandleErrorLayer,
    http::StatusCode,
    BoxError,
    Router,
};
use tower::{ServiceBuilder, timeout::TimeoutLayer};
use std::time::Duration;

async fn handler() {}

let app = Router::new()
    .route("/", get(handler))
    .layer(
        ServiceBuilder::new()
            // this middleware goes above `TimeoutLayer` because it will receive
            // errors returned by `TimeoutLayer`
            .layer(HandleErrorLayer::new(|_: BoxError| async {
                StatusCode::REQUEST_TIMEOUT
            }))
            .layer(TimeoutLayer::new(Duration::from_secs(10)))
    );
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

See [`error_handling`](crate::error_handling) for more details on axum's error
handling model.
