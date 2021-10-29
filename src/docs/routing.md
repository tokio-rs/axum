# Routing

[`Router::route`] is the main way to add routes:

```rust,no_run
use axum::{
    routing::get,
    Router,
};

let app = Router::new()
    .route("/", get(get_slash).post(post_slash))
    .route("/foo", get(get_foo));

async fn get_slash() {
    // `GET /` called
}

async fn post_slash() {
    // `POST /` called
}

async fn get_foo() {
    // `GET /foo` called
}
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Routes can also be dynamic like `/users/:id`. See [extractors](#extractors)
for more details.

You can also define routes separately and merge them with [`Router::merge`].

Routes are not allowed to overlap and will panic if an overlapping route is
added. This also means the order in which routes are added doesn't matter.

## Wildcard routes

axum also supports wildcard routes:

```rust,no_run
use axum::{
    routing::get,
    Router,
};

let app = Router::new()
    // this matches any request that starts with `/api`
    .route("/api/*rest", get(|| async { /* ... */ }));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

The matched path can be extracted via [`extract::Path`]:

```rust,no_run
use axum::{
    routing::get,
    extract::Path,
    Router,
};

let app = Router::new().route("/api/*rest", get(|Path(rest): Path<String>| async {
    // `rest` will be everything after `/api`
}));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

## Nesting routes

Routes can be nested by calling [`Router::nest`](routing::Router::nest):

```rust,no_run
use axum::{
    body::{Body, BoxBody},
    http::Request,
    routing::get,
    Router,
};
use tower_http::services::ServeFile;
use http::Response;

fn api_routes() -> Router {
    Router::new()
        .route("/users", get(|_: Request<Body>| async { /* ... */ }))
}

let app = Router::new()
    .route("/", get(|_: Request<Body>| async { /* ... */ }))
    .nest("/api", api_routes());
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Note that nested routes will not see the orignal request URI but instead
have the matched prefix stripped. This is necessary for services like static
file serving to work. Use [`OriginalUri`] if you need the original request
URI.

Nested routes are similar to wild card routes. The difference is that
wildcard routes still see the whole URI whereas nested routes will have
the prefix stripped.

```rust
use axum::{routing::get, http::Uri, Router};

let app = Router::new()
    .route("/foo/*rest", get(|uri: Uri| async {
        // `uri` will contain `/foo`
    }))
    .nest("/bar", get(|uri: Uri| async {
        // `uri` will _not_ contain `/bar`
    }));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

## Fallback routes

By default axum will respond with an empty `404 Not Found` response to unhandled requests. To
override that you can use [`Router::fallback`]:

```rust
use axum::{
    Router,
    routing::get,
    handler::Handler,
    response::IntoResponse,
    http::{StatusCode, Uri},
};

async fn fallback(uri: Uri) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, format!("No route for {}", uri))
}

let app = Router::new()
    .route("/foo", get(|| async { /* ... */ }))
    .fallback(fallback.into_service());
# async {
# hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

See [`Router::fallback`] for more details.

## Routing to any [`Service`]

axum also supports routing to general [`Service`]s:

```rust,no_run
use axum::{
    Router,
    body::Body,
    routing::service_method_router as service,
    error_handling::HandleErrorExt,
    http::{Request, StatusCode},
};
use tower_http::services::ServeFile;
use http::Response;
use std::{convert::Infallible, io};
use tower::service_fn;

let app = Router::new()
    .route(
        // Any request to `/` goes to a service
        "/",
        // Services who's response body is not `axum::body::BoxBody`
        // can be wrapped in `axum::service::any` (or one of the other routing filters)
        // to have the response body mapped
        service::any(service_fn(|_: Request<Body>| async {
            let res = Response::new(Body::from("Hi from `GET /`"));
            Ok::<_, Infallible>(res)
        }))
    )
    .route(
        "/foo",
        // This service's response body is `axum::body::BoxBody` so
        // it can be routed to directly.
        service_fn(|req: Request<Body>| async move {
            let body = Body::from(format!("Hi from `{} /foo`", req.method()));
            let body = axum::body::box_body(body);
            let res = Response::new(body);
            Ok::<_, Infallible>(res)
        })
    )
    .route(
        // GET `/static/Cargo.toml` goes to a service from tower-http
        "/static/Cargo.toml",
        service::get(ServeFile::new("Cargo.toml"))
            // though we must handle any potential errors
            .handle_error(|error: io::Error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unhandled internal error: {}", error),
                )
            })
    );
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Routing to arbitrary services in this way has complications for backpressure
([`Service::poll_ready`]). See the [`service`] module for more details.

### Routing to fallible services

Note that routing to general services has a small gotcha when it comes to
errors. axum currently does not support mixing routes to fallible services
with infallible handlers. For example this does _not_ compile:

```compile_fail
use axum::{
    Router,
    routing::{get, service_method_router as service},
    http::{Request, Response},
    body::Body,
};
use std::io;
use tower::service_fn;

let app = Router::new()
    // this route cannot fail
    .route("/foo", get(|| async {}))
    // this route can fail with io::Error
    .route(
        "/",
        service::get(service_fn(|_req: Request<Body>| async {
            let contents = tokio::fs::read_to_string("some_file").await?;
            Ok::<_, io::Error>(Response::new(Body::from(contents)))
        })),
    );
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

The solution is to use [`handle_error`] and handle the error from the
service:

```
use axum::{
    Router,
    body::Body,
    routing::{get, service_method_router as service},
    response::IntoResponse,
    http::{Request, Response},
    error_handling::HandleErrorExt,
};
use std::{io, convert::Infallible};
use tower::service_fn;

let app = Router::new()
    // this route cannot fail
    .route("/foo", get(|| async {}))
    // this route can fail with io::Error
    .route(
        "/",
        service::get(service_fn(|_req: Request<Body>| async {
            let contents = tokio::fs::read_to_string("some_file").await?;
            Ok::<_, io::Error>(Response::new(Body::from(contents)))
        }))
        .handle_error(handle_io_error),
    );

fn handle_io_error(error: io::Error) -> impl IntoResponse {
    // ...
}
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

In this particular case you can also handle the error directly in
`service_fn` but that is not possible, if you're routing to a service which
you don't control.

See ["Error handling"](#error-handling) for more details on [`handle_error`]
and error handling in general.
