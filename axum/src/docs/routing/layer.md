Apply a [`tower::Layer`] to the router.

All requests to the router will be processed by the layer's
corresponding middleware.

This can be used to add additional processing to a request for a group
of routes.

Note this differs from [`Handler::layer`](crate::handler::Handler::layer)
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

# Multiple middleware

It's recommended to use [`tower::ServiceBuilder`] when applying multiple
middleware. See [`middleware`](crate::middleware) for more details.

# Error handling

See [`middleware`](crate::middleware) for details on how error handling impacts
middleware.
