Apply a [`tower::Layer`] to the router.

All requests to the router will be processed by the layer's
corresponding middleware.

This can be used to add additional processing to a request for a group
of routes.

Works similarly to [`Router::layer`](super::Router::layer). See that method for
more details.

# Example

```rust
use axum::{routing::get, Router};
use tower::limit::ConcurrencyLimitLayer;

async fn hander() {}

let app = Router::new().route(
    "/",
    // All requests to `GET /` will be sent through `ConcurrencyLimitLayer`
    get(hander).layer(ConcurrencyLimitLayer::new(64)),
);
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```
