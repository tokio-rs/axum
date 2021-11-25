Types and traits for generating responses.

# Building responses

Anything that implements [`IntoResponse`] can be returned from a handler:

```rust,no_run
use axum::{
    body::Body,
    routing::get,
    handler::Handler,
    http::{Request, header::{HeaderMap, HeaderName, HeaderValue}},
    response::{IntoResponse, Html, Json, Headers},
    Router,
};
use http::{StatusCode, Response, Uri};
use serde_json::{Value, json};

// We've already seen returning &'static str
async fn plain_text() -> &'static str {
    "foo"
}

// String works too and will get a `text/plain; charset=utf-8` content-type
async fn plain_text_string(uri: Uri) -> String {
    format!("Hi from {}", uri.path())
}

// Bytes will get a `application/octet-stream` content-type
async fn bytes() -> Vec<u8> {
    vec![1, 2, 3, 4]
}

// `()` gives an empty response
async fn empty() {}

// `StatusCode` gives an empty response with that status code
async fn empty_with_status() -> StatusCode {
    StatusCode::NOT_FOUND
}

// A tuple of `StatusCode` and something that implements `IntoResponse` can
// be used to override the status code
async fn with_status() -> (StatusCode, &'static str) {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong")
}

// A tuple of `HeaderMap` and something that implements `IntoResponse` can
// be used to override the headers
async fn with_headers() -> (HeaderMap, &'static str) {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-foo"),
        HeaderValue::from_static("foo"),
    );
    (headers, "foo")
}

// You can also override both status and headers at the same time
async fn with_headers_and_status() -> (StatusCode, HeaderMap, &'static str) {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-foo"),
        HeaderValue::from_static("foo"),
    );
    (StatusCode::INTERNAL_SERVER_ERROR, headers, "foo")
}

// `Headers` makes building the header map easier and `impl Trait` is easier
// so you don't have to write the whole type
async fn with_easy_headers() -> impl IntoResponse {
    Headers(vec![("x-foo", "foo")])
}

// `Html` gives a content-type of `text/html`
async fn html() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

// `Json` gives a content-type of `application/json` and works with any type
// that implements `serde::Serialize`
async fn json() -> Json<Value> {
    Json(json!({ "data": 42 }))
}

// `Result<T, E>` where `T` and `E` implement `IntoResponse` is useful for
// returning errors
async fn result() -> Result<&'static str, StatusCode> {
    Ok("all good")
}

// `Response` gives full control
async fn response() -> Response<Body> {
    Response::builder().body(Body::empty()).unwrap()
}

let app = Router::new()
    .route("/plain_text", get(plain_text))
    .route("/plain_text_string", get(plain_text_string))
    .route("/bytes", get(bytes))
    .route("/empty", get(empty))
    .route("/empty_with_status", get(empty_with_status))
    .route("/with_status", get(with_status))
    .route("/with_headers", get(with_headers))
    .route("/with_headers_and_status", get(with_headers_and_status))
    .route("/with_easy_headers", get(with_easy_headers))
    .route("/html", get(html))
    .route("/json", get(json))
    .route("/result", get(result))
    .route("/response", get(response));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```
