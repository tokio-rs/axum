Add a fallback service to the router.

This service will be called if no routes matches the incoming request.

```rust
use axum::{
    Router,
    routing::get,
    handler::Handler,
    response::IntoResponse,
    http::{StatusCode, Uri},
};

let app = Router::new()
    .route("/foo", get(|| async { /* ... */ }))
    .fallback(fallback.into_service());

async fn fallback(uri: Uri) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, format!("No route for {}", uri))
}
# async {
# hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Fallbacks only apply to routes that aren't matched by anything in the
router. If a handler is matched by a request but returns 404 the
fallback is not called.

## When used with `Router::merge`

If a router with a fallback is merged with another router that also has
a fallback the fallback of the second router takes precedence:

```rust
use axum::{
    Router,
    routing::get,
    handler::Handler,
    response::IntoResponse,
    http::{StatusCode, Uri},
};

let one = Router::new()
    .route("/one", get(|| async {}))
    .fallback(fallback_one.into_service());

let two = Router::new()
    .route("/two", get(|| async {}))
    .fallback(fallback_two.into_service());

let app = one.merge(two);

async fn fallback_one() -> impl IntoResponse {}
async fn fallback_two() -> impl IntoResponse {}

// the fallback for `app` is `fallback_two`
# async {
# hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

If only one of the routers have a fallback that will be used in the
merged router.

## When used with `Router::nest`

If a router with a fallback is nested inside another router the fallback
of the nested router will be discarded and not used. This is such that
the outer router's fallback takes precedence.
