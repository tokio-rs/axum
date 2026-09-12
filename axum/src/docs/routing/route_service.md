Add another route to the router that calls a [`Service`].

Unlike [`Router::route`], this accepts a [`Service`] directly rather than a
[`MethodRouter`]. All HTTP methods are forwarded to the service.

The service receives a [`Request`] (whose body type is [`Body`]) and must
return a response that implements [`IntoResponse`]. The error type must be
[`Infallible`].

[`any_service`], [`get_service`], and the other `*_service` filters turn a
[`Service`] into a [`MethodRouter`]. Use those with [`Router::route`] when you
want method-based routing, or call the [`MethodRouter`] as a [`Service`]
without a `Router`. You do **not** need them to map response bodies — any
[`IntoResponse`] type works with `route_service` directly.

# Example

```rust
use axum::{
    Router,
    body::Body,
    routing::any_service,
    extract::Request,
};
use http::Response;
use std::convert::Infallible;
use tower::service_fn;
use tower_http::services::ServeFile;

let app = Router::new()
    .route(
        "/",
        // `route` takes a `MethodRouter`. `any_service` matches all methods;
        // use `get_service` / `post_service` / ... to match specific methods.
        any_service(service_fn(|_: Request| async {
            let res = Response::new(Body::from("Hi from `/`"));
            Ok::<_, Infallible>(res)
        })),
    )
    .route_service(
        "/foo",
        // `route_service` takes any `Service<Request>` whose response
        // implements `IntoResponse`. No body boxing is required.
        service_fn(|req: Request| async move {
            let body = Body::from(format!("Hi from `{} /foo`", req.method()));
            let res = Response::new(body);
            Ok::<_, Infallible>(res)
        }),
    )
    .route_service(
        // Services from tower-http work the same way.
        "/static/Cargo.toml",
        ServeFile::new("Cargo.toml"),
    );
# let _: Router = app;
```

Routing to arbitrary services in this way has complications for backpressure
([`Service::poll_ready`]). See the [Routing to services and backpressure] module
for more details.

# Panics

Panics for the same reasons as [`Router::route`] or if you attempt to route to a
`Router`:

```rust,should_panic
use axum::{routing::get, Router};

let app = Router::new().route_service(
    "/",
    Router::new().route("/foo", get(|| async {})),
);
# let _: Router = app;
```

Use [`Router::nest`] instead.

[Routing to services and backpressure]: middleware/index.html#routing-to-servicesmiddleware-and-backpressure
