# Extractors

An extractor is a type that implements [`FromRequest`]. Extractors is how
you pick apart the incoming request to get the parts your handler needs.

For example, [`extract::Json`] is an extractor that consumes the request
body and deserializes it as JSON into some target type:

```rust,no_run
use axum::{
    extract::Json,
    routing::post,
    Router,
};
use serde::Deserialize;

let app = Router::new().route("/users", post(create_user));

#[derive(Deserialize)]
struct CreateUser {
    email: String,
    password: String,
}

async fn create_user(Json(payload): Json<CreateUser>) {
    // ...
}
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

See the [`extract`] module for everything that can be used as an extractor.

## Common extractors

Some commonly used extractors are:

```rust,no_run
use axum::{
    extract::{Json, TypedHeader, Path, Extension, Query},
    routing::post,
    http::{Request, header::HeaderMap},
    body::{Bytes, Body},
    Router,
};
use serde_json::Value;
use headers::UserAgent;
use std::collections::HashMap;

// `Path` gives you the path parameters and deserializes them. See its docs for
// more details
async fn path(Path(user_id): Path<u32>) {}

// `Query` gives you the query parameters and deserializes them.
async fn query(Query(params): Query<HashMap<String, String>>) {}

// `HeaderMap` gives you all the headers
async fn headers(headers: HeaderMap) {}

// `TypedHeader` can be used to extract a single header
// note this requires you've enabled axum's `headers`
async fn user_agent(TypedHeader(user_agent): TypedHeader<UserAgent>) {}

// `String` consumes the request body and ensures it is valid utf-8
async fn string(body: String) {}

// `Bytes` gives you the raw request body
async fn bytes(body: Bytes) {}

// We've already seen `Json` for parsing the request body as json
async fn json(Json(payload): Json<Value>) {}

// `Request` gives you the whole request for maximum control
async fn request(request: Request<Body>) {}

// `Extension` extracts data from "request extensions"
// See the "Sharing state with handlers" section for more details
async fn extension(Extension(state): Extension<State>) {}

#[derive(Clone)]
struct State { /* ... */ }

let app = Router::new()
    .route("/path", post(path))
    .route("/query", post(query))
    .route("/user_agent", post(user_agent))
    .route("/headers", post(headers))
    .route("/string", post(string))
    .route("/bytes", post(bytes))
    .route("/json", post(json))
    .route("/request", post(request))
    .route("/extension", post(extension));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

## Applying multiple extractors

You can also apply multiple extractors:

```rust,no_run
use axum::{
    extract,
    routing::get,
    Router,
};
use uuid::Uuid;
use serde::Deserialize;

let app = Router::new().route("/users/:id/things", get(get_user_things));

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
    extract::Path(user_id): extract::Path<Uuid>,
    pagination: Option<extract::Query<Pagination>>,
) {
    let pagination: Pagination = pagination.unwrap_or_default().0;

    // ...
}
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Take care of the order in which you apply extractors as some extractors
mutate the request.

For example using [`HeaderMap`] as an extractor will make the headers
inaccessible for other extractors on the handler. If you need to extract
individual headers _and_ a [`HeaderMap`] make sure to apply the extractor of
individual headers first:

```rust,no_run
use axum::{
    extract::TypedHeader,
    routing::get,
    http::header::HeaderMap,
    Router,
};
use headers::UserAgent;

async fn handler(
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    all_headers: HeaderMap,
) {
    // ...
}

let app = Router::new().route("/", get(handler));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Extractors that consume the request body can also only be applied once as
well as [`Request`], which consumes the entire request:

```rust,no_run
use axum::{
    routing::get,
    http::Request,
    body::Body,
    Router,
};

async fn handler(request: Request<Body>) {
    // ...
}

let app = Router::new().route("/", get(handler));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Extractors always run in the order of the function parameters that is from
left to right.

## Optional extractors

All extractors defined in axum will reject the request if it doesn't match.
If you wish to make an extractor optional you can wrap it in `Option`:

```rust,no_run
use axum::{
    extract::Json,
    routing::post,
    Router,
};
use serde_json::Value;

async fn create_user(payload: Option<Json<Value>>) {
    if let Some(payload) = payload {
        // We got a valid JSON payload
    } else {
        // Payload wasn't valid JSON
    }
}

let app = Router::new().route("/users", post(create_user));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Wrapping extractors in `Result` makes them optional and gives you the reason
the extraction failed:

```rust,no_run
use axum::{
    extract::{Json, rejection::JsonRejection},
    routing::post,
    Router,
};
use serde_json::Value;

async fn create_user(payload: Result<Json<Value>, JsonRejection>) {
    match payload {
        Ok(payload) => {
            // We got a valid JSON payload
        }
        Err(JsonRejection::MissingJsonContentType(_)) => {
            // Request didn't have `Content-Type: application/json`
            // header
        }
        Err(JsonRejection::InvalidJsonBody(_)) => {
            // Couldn't deserialize the body into the target type
        }
        Err(JsonRejection::BodyAlreadyExtracted(_)) => {
            // Another extractor had already consumed the body
        }
        Err(_) => {
            // `JsonRejection` is marked `#[non_exhaustive]` so match must
            // include a catch-all case.
        }
    }
}

let app = Router::new().route("/users", post(create_user));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

## Customizing extractor responses

If an extractor fails it will return a response with the error and your
handler will not be called. To customize the error response you have a two
options:

1. Use `Result<T, T::Rejection>` as your extractor like shown in ["Optional
   extractors"](#optional-extractors). This works well if you're only using
   the extractor in a single handler.
2. Create your own extractor that in its [`FromRequest`] implemention calls
   one of axum's built in extractors but returns a different response for
   rejections. See the [customize-extractor-error] example for more details.
