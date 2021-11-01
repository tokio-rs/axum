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

// String works too and will get a `text/plain` content-type
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

# Implementing `IntoResponse`

You generally shouldn't have to implement `IntoResponse` manually, as axum
provides implementations for many common types.

However it might be necessary if you have a custom error type that you want
to return from handlers:

```rust
use axum::{
    Router,
    body::Body,
    routing::get,
    http::{Response, StatusCode},
    response::IntoResponse,
};

enum MyError {
    SomethingWentWrong,
    SomethingElseWentWrong,
}

impl IntoResponse for MyError {
    type Body = Body;
    type BodyError = <Self::Body as axum::body::HttpBody>::Error;

    fn into_response(self) -> Response<Self::Body> {
        let body = match self {
            MyError::SomethingWentWrong => {
                Body::from("something went wrong")
            },
            MyError::SomethingElseWentWrong => {
                Body::from("something else went wrong")
            },
        };

        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(body)
            .unwrap()
    }
}

// `Result<impl IntoResponse, MyError>` can now be returned from handlers
let app = Router::new().route("/", get(handler));

async fn handler() -> Result<(), MyError> {
    Err(MyError::SomethingWentWrong)
}
# async {
# hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Or if you have a custom body type you'll also need to implement
`IntoResponse` for it:

```rust
use axum::{
    routing::get,
    response::IntoResponse,
    Router,
};
use http_body::Body;
use http::{Response, HeaderMap};
use bytes::Bytes;
use std::{
    convert::Infallible,
    task::{Poll, Context},
    pin::Pin,
};

struct MyBody;

// First implement `Body` for `MyBody`. This could for example use
// some custom streaming protocol.
impl Body for MyBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        # unimplemented!()
        // ...
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        # unimplemented!()
        // ...
    }
}

// Now we can implement `IntoResponse` directly for `MyBody`
impl IntoResponse for MyBody {
    type Body = Self;
    type BodyError = <Self as Body>::Error;

    fn into_response(self) -> Response<Self::Body> {
        Response::new(self)
    }
}

// We don't need to implement `IntoResponse for Response<MyBody>` as that is
// covered by a blanket implementation in axum.

// `MyBody` can now be returned from handlers.
let app = Router::new().route("/", get(|| async { MyBody }));
# async {
# hyper::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```
