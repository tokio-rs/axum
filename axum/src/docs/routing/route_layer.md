Apply a [`tower::Layer`] to the router that will only run if the request matches
a route.

Note that the middleware is only applied to existing routes. So you have to
first add your routes and then call `route_layer` afterwards. Additional routes
added after `route_layer` is called will not have the middleware added.

**Important**: The middleware is NOT applied to fallback handlers. Fallback handlers
will run without the middleware applied. This is because `route_layer` only applies
to matched requests, while fallbacks handle unmatched requests.

This works similarly to [`Router::layer`] except the middleware will only run if
the request matches a route. This is useful for middleware that return early
(such as authorization) which might otherwise convert a `404 Not Found` into a
`401 Unauthorized`.

This function will panic if no routes have been declared yet on the router,
since the new layer will have no effect, and this is typically a bug.
In generic code, you can test if that is the case first, by calling [`Router::has_routes`].

# Example

```rust
use axum::{
    routing::get,
    Router,
};
use tower_http::validate_request::ValidateRequestHeaderLayer;

let app = Router::new()
    .route("/foo", get(|| async {}))
    .fallback(|| async { "fallback" })
    .route_layer(ValidateRequestHeaderLayer::bearer("password"));

// `GET /foo` with a valid token will receive `200 OK`
// `GET /foo` with a invalid token will receive `401 Unauthorized`
// `GET /not-found` with a invalid token will receive `200 OK` (fallback)
# let _: Router = app;
```
