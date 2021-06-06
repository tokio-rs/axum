# tower-web

tower-web (name pending) is a tiny web application framework that focuses on
ergonimics and modularity.

### Goals

- Ease of use. Build web apps in Rust should be as easy as `async fn
handle(Request) -> Response`.
- Solid foundation. tower-web is built on top of tower and makes it easy to
plug in any middleware from the [tower] and [tower-http] ecosystem.
- Focus on routing, extracing data from requests, and generating responses.
tower middleware can handle the rest.
- Macro free core. Macro frameworks have their place but tower-web focuses
on providing a core that is macro free.

### Non-goals

- Runtime independent. tower-web is designed to work with tokio and hyper
and focused on bringing a good to experience to that stack.
- Speed. tower-web is a of course a fast framework, and wont be the
bottleneck in your app, but the goal is not to top the benchmarks.

## Example

The "Hello, World!" of tower-web is:

```rust
use tower_web::prelude::*;
use hyper::Server;
use std::net::SocketAddr;
use tower::make::Shared;

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = route("/", get(handler));

    // run it with hyper on localhost:3000
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let server = Server::bind(&addr).serve(Shared::new(app));
    server.await.unwrap();
}

async fn handler(req: Request<Body>) -> &'static str {
    "Hello, World!"
}
```

## Routing

Routing between handlers looks like this:

```rust
use tower_web::prelude::*;

let app = route("/", get(get_slash).post(post_slash))
    .route("/foo", get(get_foo));

async fn get_slash(req: Request<Body>) {
    // `GET /` called
}

async fn post_slash(req: Request<Body>) {
    // `POST /` called
}

async fn get_foo(req: Request<Body>) {
    // `GET /foo` called
}
```

Routes can also be dynamic like `/users/:id`. See ["Extracting data from
requests"](#extracting-data-from-requests) for more details on that.

## Responses

Anything that implements [`IntoResponse`] can be returned from a handler:

```rust
use tower_web::{body::Body, response::{Html, Json}, prelude::*};
use http::{StatusCode, Response};
use serde_json::{Value, json};

// We've already seen returning &'static str
async fn plain_text(req: Request<Body>) -> &'static str {
    "foo"
}

// String works too and will get a text/plain content-type
async fn plain_text_string(req: Request<Body>) -> String {
    format!("Hi from {}", req.uri().path())
}

// Bytes will get a `application/octet-stream` content-type
async fn bytes(req: Request<Body>) -> Vec<u8> {
    vec![1, 2, 3, 4]
}

// `()` gives an empty response
async fn empty(req: Request<Body>) {}

// `StatusCode` gives an empty response with that status code
async fn empty_with_status(req: Request<Body>) -> StatusCode {
    StatusCode::NOT_FOUND
}

// A tuple of `StatusCode` and something that implements `IntoResponse` can
// be used to override the status code
async fn with_status(req: Request<Body>) -> (StatusCode, &'static str) {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong")
}

// `Html` gives a content-type of `text/html`
async fn html(req: Request<Body>) -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

// `Json` gives a content-type of `application/json` and works with my type
// that implements `serde::Serialize`
async fn json(req: Request<Body>) -> Json<Value> {
    Json(json!({ "data": 42 }))
}

// `Result<T, E>` where `T` and `E` implement `IntoResponse` is useful for
// returning errors
async fn result(req: Request<Body>) -> Result<&'static str, StatusCode> {
    Ok("all good")
}

// `Response` gives full control
async fn response(req: Request<Body>) -> Response<Body> {
    Response::builder().body(Body::empty()).unwrap()
}

let app = route("/plain_text", get(plain_text))
    .route("/plain_text_string", get(plain_text_string))
    .route("/bytes", get(bytes))
    .route("/empty", get(empty))
    .route("/empty_with_status", get(empty_with_status))
    .route("/with_status", get(with_status))
    .route("/html", get(html))
    .route("/json", get(json))
    .route("/result", get(result))
    .route("/response", get(response));
```

See the [`response`] module for more details.

## Extracting data from requests

A handler function must always take `Request<Body>` as its first argument
but any arguments following are called "extractors". Any type that
implements [`FromRequest`](crate::extract::FromRequest) can be used as an
extractor.

[`extract::Json`] is an extractor that consumes the request body and
deserializes as as JSON into some target type:

```rust
use tower_web::prelude::*;
use serde::Deserialize;

let app = route("/users", post(create_user));

#[derive(Deserialize)]
struct CreateUser {
    email: String,
    password: String,
}

async fn create_user(req: Request<Body>, payload: extract::Json<CreateUser>) {
    let payload: CreateUser = payload.0;

    // ...
}
```

[`extract::UrlParams`] can be used to extract params from a dynamic URL. It
is compatible with any type that implements [`std::str::FromStr`], such as
[`Uuid`]:

```rust
use tower_web::prelude::*;
use uuid::Uuid;

let app = route("/users/:id", post(create_user));

async fn create_user(req: Request<Body>, params: extract::UrlParams<(Uuid,)>) {
    let (user_id,) = params.0;

    // ...
}
```

There is also [`UrlParamsMap`](extract::UrlParamsMap) which provide a map
like API for extracting URL params.

You can also apply multiple extractors:

```rust
use tower_web::prelude::*;
use uuid::Uuid;
use serde::Deserialize;

let app = route("/users/:id/things", get(get_user_things));

#[derive(Deserialize)]
struct Pagination {
    page: usize,
    per_page: usize,
}

impl Default for Pagination {
    fn default() -> Self {
        Self { page: 1, per_page: 30 }
    }
}

async fn get_user_things(
    req: Request<Body>,
    params: extract::UrlParams<(Uuid,)>,
    pagination: Option<extract::Query<Pagination>>,
) {
    let user_id: Uuid = (params.0).0;
    let pagination: Pagination = pagination.unwrap_or_default().0;

    // ...
}
```

See the [`extract`] module for more details.

[`Uuid`]: https://docs.rs/uuid/latest/uuid/

## Applying middleware

tower-web is designed to take full advantage of the tower and tower-http
ecosystem of middleware:

### To individual handlers

A middleware can be applied to a single handler like so:

```rust
use tower_web::prelude::*;
use tower::limit::ConcurrencyLimitLayer;

let app = route(
    "/",
    get(handler.layer(ConcurrencyLimitLayer::new(100))),
);

async fn handler(req: Request<Body>) {}
```

### To groups of routes

Middleware can also be applied to a group of routes like so:

```rust
use tower_web::prelude::*;
use tower::limit::ConcurrencyLimitLayer;

let app = route("/", get(get_slash))
    .route("/foo", post(post_foo))
    .layer(ConcurrencyLimitLayer::new(100));

async fn get_slash(req: Request<Body>) {}

async fn post_foo(req: Request<Body>) {}
```

### Error handling

tower-web requires all errors to be handled. That is done by using
[`std::convert::Infallible`] as the error type in all its [`Service`]
implementations.

For handlers created from async functions this is works automatically since
handlers must return something that implements [`IntoResponse`], even if its
a `Result`.

However middleware might add new failure cases that has to be handled. For
that tower-web provides a `handle_error` combinator:

```rust
use tower_web::prelude::*;
use tower::{
    BoxError, timeout::{TimeoutLayer, error::Elapsed},
};
use std::{borrow::Cow, time::Duration};
use http::StatusCode;

let app = route(
    "/",
    get(handle
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        // `Timeout` uses `BoxError` as the error type
        .handle_error(|error: BoxError| {
            // Check if the actual error type is `Elapsed` which
            // `Timeout` returns
            if error.is::<Elapsed>() {
                return (StatusCode::REQUEST_TIMEOUT, "Request took too long".into());
            }

            // If we encounter some error we don't handle return a generic
            // error
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                // `Cow` lets us return either `&str` or `String`
                Cow::from(format!("Unhandled internal error: {}", error)),
            );
        })),
);

async fn handle(req: Request<Body>) {}
```

The closure passed to `handle_error` must return something that implements
`IntoResponse`.

`handle_error` is also available on a group of routes with middleware
applied:

```rust
use tower_web::prelude::*;
use tower::{
    BoxError, timeout::{TimeoutLayer, error::Elapsed},
};
use std::{borrow::Cow, time::Duration};
use http::StatusCode;

let app = route("/", get(handle))
    .layer(TimeoutLayer::new(Duration::from_secs(30)))
    .handle_error(|error: BoxError| {
        // ...
    });

async fn handle(req: Request<Body>) {}
```

### Applying multiple middleware

[`tower::ServiceBuilder`] can be used to combine multiple middleware:

```rust
use tower_web::prelude::*;
use tower::{
    ServiceBuilder, BoxError,
    load_shed::error::Overloaded,
    timeout::error::Elapsed,
};
use tower_http::compression::CompressionLayer;
use std::{borrow::Cow, time::Duration};
use http::StatusCode;

let middleware_stack = ServiceBuilder::new()
    // Return an error after 30 seconds
    .timeout(Duration::from_secs(30))
    // Shed load if we're receiving too many requests
    .load_shed()
    // Process at most 100 requests concurrently
    .concurrency_limit(100)
    // Compress response bodies
    .layer(CompressionLayer::new())
    .into_inner();

let app = route("/", get(|_: Request<Body>| async { /* ... */ }))
    .layer(middleware_stack)
    .handle_error(|error: BoxError| {
        if error.is::<Overloaded>() {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Try again later".into(),
            );
        }

        if error.is::<Elapsed>() {
            return (
                StatusCode::REQUEST_TIMEOUT,
                "Request took too long".into(),
            );
        };

        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Cow::from(format!("Unhandled internal error: {}", error)),
        );
    });
```

## Sharing state with handlers

It is common to share some state between handlers for example to share a
pool of database connections or clients to other services. That can be done
using the [`AddExtension`] middleware (applied with [`AddExtensionLayer`])
and the [`extract::Extension`] extractor:

```rust
use tower_web::{AddExtensionLayer, prelude::*};
use std::sync::Arc;

struct State {
    // ...
}

let shared_state = Arc::new(State { /* ... */ });

let app = route("/", get(handler)).layer(AddExtensionLayer::new(shared_state));

async fn handler(
    req: Request<Body>,
    state: extract::Extension<Arc<State>>,
) {
    let state: Arc<State> = state.0;

    // ...
}
```

## Routing to any [`Service`]

tower-web also supports routing to general [`Service`]s:

```rust
use tower_web::{
    service, prelude::*,
    // `ServiceExt` adds `handle_error` to any `Service`
    ServiceExt,
};
use tower_http::services::ServeFile;
use http::Response;
use std::convert::Infallible;
use tower::{service_fn, BoxError};

let app = route(
    // Any request to `/` goes to a service
    "/",
    service_fn(|_: Request<Body>| async {
        let res = Response::new(Body::from("Hi from `GET /`"));
        Ok::<_, Infallible>(res)
    })
).route(
    // GET `/static/Cargo.toml` goes to a service from tower-http
    "/static/Cargo.toml",
    service::get(
        ServeFile::new("Cargo.toml")
            // Errors must be handled
            .handle_error(|error: std::io::Error| { /* ... */ })
    )
);
```

See the [`service`] module for more details.

## Nesting applications

TODO

[tower]: https://crates.io/crates/tower
[tower-http]: https://crates.io/crates/tower-http
