Apply a [`tower::Layer`] to the router that will only run if the request matches
a route.

This works similarly to [`MethodRouter::layer`] except the middleware will only run if
the request matches a route. This is useful for middleware that return early
(such as authorization) which might otherwise convert a `405 Method Not Allowed` into a
`401 Unauthorized`.

# Example

```rust
use axum::{
    routing::get,
    Router,
};
use tower_http::auth::RequireAuthorizationLayer;

let app = Router::new().route(
    "/foo",
    get(|| async {})
        .route_layer(RequireAuthorizationLayer::bearer("password"))
);

// `GET /foo` with a valid token will receive `200 OK`
// `GET /foo` with a invalid token will receive `401 Unauthorized`
// `POST /FOO` with a invalid token will receive `405 Method Not Allowed`
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```
