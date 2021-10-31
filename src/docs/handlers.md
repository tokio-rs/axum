# Handlers

In axum a "handler" is an async function that accepts zero or more
["extractors"](#extractors) as arguments and returns something that
can be converted [into a response](#building-responses).

Handlers is where your custom domain logic lives and axum applications are
built by routing between handlers.

Some examples of handlers:

```rust
use bytes::Bytes;
use http::StatusCode;

// Handler that immediately returns an empty `200 OK` response.
async fn unit_handler() {}

// Handler that immediately returns an empty `200 OK` response with a plain
// text body.
async fn string_handler() -> String {
    "Hello, World!".to_string()
}

// Handler that buffers the request body and returns it.
async fn echo(body: Bytes) -> Result<String, StatusCode> {
    if let Ok(string) = String::from_utf8(body.to_vec()) {
        Ok(string)
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}
```

## Debugging handler type errors

For a function to used as a handler it must implement the [`Handler`] trait.
axum provides blanket implementations for functions that:

- Are `async fn`s.
- Take no more than 16 arguments that all implement [`FromRequest`].
- Returns something that implements [`IntoResponse`].
- If a closure is used it must implement `Clone + Send + Sync` and be
`'static`.
- Returns a future that is `Send`. The most common way to accidentally make a
future `!Send` is to hold a `!Send` type across an await.

Unfortunately Rust gives poor error messages if you try to use a function
that doesn't quite match what's required by [`Handler`].

You might get an error like this:

```not_rust
error[E0277]: the trait bound `fn(bool) -> impl Future {handler}: Handler<_, _>` is not satisfied
   --> src/main.rs:13:44
    |
13  |     let app = Router::new().route("/", get(handler));
    |                                            ^^^^^^^ the trait `Handler<_, _>` is not implemented for `fn(bool) -> impl Future {handler}`
    |
   ::: axum/src/handler/mod.rs:116:8
    |
116 |     H: Handler<B, T>,
    |        ------------- required by this bound in `axum::routing::get`
```

This error doesn't tell you _why_ your function doesn't implement
[`Handler`]. It's possible to improve the error with the [`debug_handler`]
proc-macro from the [axum-debug] crate.
