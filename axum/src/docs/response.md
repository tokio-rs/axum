Types and traits for generating responses.

# Building responses

Anything that implements [`IntoResponse`] can be returned from a handler. axum
provides implementations for common types:

```rust,no_run
use axum::{
    Json,
    response::{Html, IntoResponse},
    http::{StatusCode, Uri, header::{self, HeaderMap, HeaderName}},
};

// `()` gives an empty response
async fn empty() {}

// String get a `text/plain; charset=utf-8` content-type
async fn plain_text(uri: Uri) -> String {
    format!("Hi from {}", uri.path())
}

// Bytes will get a `application/octet-stream` content-type
async fn bytes() -> Vec<u8> {
    vec![1, 2, 3, 4]
}

// `Json` will get a `application/json` content-type and work with anything that
// implements `serde::Serialize`
async fn json() -> Json<Vec<String>> {
    Json(vec!["foo".to_owned(), "bar".to_owned()])
}

// `Html` will get a `text/html` content-type
async fn html() -> Html<&'static str> {
    Html("<p>Hello, World!</p>")
}

// `StatusCode` gives an empty response with that status code
async fn status() -> StatusCode {
    StatusCode::NOT_FOUND
}

// `HeaderMap` gives an empty response with some headers
async fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::SERVER, "axum".parse().unwrap());
    headers
}

// An array of tuples also gives headers
async fn array_headers() -> [(HeaderName, &'static str); 2] {
    [
        (header::SERVER, "axum"),
        (header::CONTENT_TYPE, "text/plain")
    ]
}

// Use `impl IntoResponse` to avoid writing the whole type
async fn impl_trait() -> impl IntoResponse {
    [
        (header::SERVER, "axum"),
        (header::CONTENT_TYPE, "text/plain")
    ]
}
```

Additionally you can return tuples to build more complex responses from
individual parts. Each element, from left to right, will set a part of the
response:

```rust,no_run
use axum::{
    Json,
    response::IntoResponse,
    http::{StatusCode, HeaderMap, Uri, header},
    extract::Extension,
};

// A `404 Not Found` response with a `text/plain` body
async fn with_status(uri: Uri) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, format!("Not Found: {}", uri.path()))
}

// The order doesn't matter
async fn with_status_reverse(uri: Uri) -> (String, StatusCode) {
    (format!("Not Found: {}", uri.path()), StatusCode::NOT_FOUND)
}

// Use `impl IntoResponse` to avoid having to type the whole type
async fn impl_trait(uri: Uri) -> impl IntoResponse {
    (format!("Not Found: {}", uri.path()), StatusCode::NOT_FOUND)
}

// Any response parts that works on its own, also works in a tuple as
// part of a larger response.
//
// This returns
//
//     404 Not Found
//     x-foo: custom-header
//     content-type: application/json
//
//     {"error":"not found"}
//
// With a response extension accessible to middleware
async fn with_status_headers_and_body() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        [("x-foo", "custom-header")],
        Json(serde_json::json!({ "error": "not found" })),
        Extension(Foo("foo"))
    )
}

#[derive(Clone)]
struct Foo(&'static str);
```

Use [`Response`](crate::response::Response) for more low level control:

```rust,no_run
use axum::{
    Json,
    response::{IntoResponse, Response},
    body::Full,
    http::StatusCode,
};

async fn response() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("x-foo", "custom header")
        .body(Full::from("not found"))
        .unwrap()
}
```

# `IntoResponseParts` and `IntoResponse`

Building responses works via two traits:

- [`IntoResponseParts`]: A trait for modifying a response and setting one or
  more parts of it. Implement this trait to define new response types.
- [`IntoResponse`]: A complete response that can be sent from a handler. You
  cannot implement this trait directly.

Note that [`IntoResponse`] is _sealed_ meaning you cannot implement it. Instead
you must implement [`IntoResponseParts`] which has the blanked implementation
`impl<T: IntoResponseParts> IntoResponse for T`. This means that any type that
implements [`IntoResponseParts`] automatically also implements [`IntoResponse`].
This is why types such as [`StatusCode`] which implement [`IntoResponseParts`]
can be used as `impl IntoResponse` and be returned from handlers.

See the docs for [`IntoResponseParts`] for details on how to implement it.

It is recommended that you familiarize yourself with everything that implements
[`IntoResponseParts`].

[`IntoResponse`]: crate::response::IntoResponse
[`IntoResponseParts`]: crate::response::IntoResponseParts
[`StatusCode`]: http::StatusCode
