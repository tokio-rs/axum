# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Unreleased

- Add accessors to TypedHeaderRejection fields ([#317])

[#317]: https://github.com/tokio-rs/axum/pull/317

# 0.2.4 (10. September, 2021)

- Document using `StreamExt::split` with `WebSocket` ([#291])
- Document adding middleware to multiple groups of routes ([#293])

[#291]: https://github.com/tokio-rs/axum/pull/291
[#293]: https://github.com/tokio-rs/axum/pull/293

# 0.2.3 (26. August, 2021)

- **fixed:** Fix accidental breaking change introduced by internal refactor.
  `BoxRoute` used to be `Sync` but was accidental made `!Sync` ([#273](https://github.com/tokio-rs/axum/pull/273))

# 0.2.2 (26. August, 2021)

- **fixed:** Fix URI captures matching empty segments. This means requests with
  URI `/` will no longer be matched by `/:key` ([#264](https://github.com/tokio-rs/axum/pull/264))
- **fixed:** Remove needless trait bounds from `Router::boxed` ([#269](https://github.com/tokio-rs/axum/pull/269))

# 0.2.1 (24. August, 2021)

- **added:** Add `Redirect::to` constructor ([#255](https://github.com/tokio-rs/axum/pull/255))
- **added:** Document how to implement `IntoResponse` for custom error type ([#258](https://github.com/tokio-rs/axum/pull/258))

# 0.2.0 (23. August, 2021)

- Overall:
  - **fixed:** Overall compile time improvements. If you're having issues with compile time
    please file an issue! ([#184](https://github.com/tokio-rs/axum/pull/184)) ([#198](https://github.com/tokio-rs/axum/pull/198)) ([#220](https://github.com/tokio-rs/axum/pull/220))
  - **changed:** Remove `prelude`. Explicit imports are now required ([#195](https://github.com/tokio-rs/axum/pull/195))
- Routing:
  - **added:** Add dedicated `Router` to replace the `RoutingDsl` trait ([#214](https://github.com/tokio-rs/axum/pull/214))
  - **added:** Add `Router::or` for combining routes ([#108](https://github.com/tokio-rs/axum/pull/108))
  - **fixed:** Support matching different HTTP methods for the same route that aren't defined
    together. So `Router::new().route("/", get(...)).route("/", post(...))` now
    accepts both `GET` and `POST`. Previously only `POST` would be accepted ([#224](https://github.com/tokio-rs/axum/pull/224))
  - **fixed:** `get` routes will now also be called for `HEAD` requests but will always have
    the response body removed ([#129](https://github.com/tokio-rs/axum/pull/129))
  - **changed:** Replace `axum::route(...)` with `axum::Router::new().route(...)`. This means
    there is now only one way to create a new router. Same goes for
    `axum::routing::nest`. ([#215](https://github.com/tokio-rs/axum/pull/215))
  - **changed:** Implement `routing::MethodFilter` via [`bitflags`](https://crates.io/crates/bitflags) ([#158](https://github.com/tokio-rs/axum/pull/158))
  - **changed:** Move `handle_error` from `ServiceExt` to `service::OnMethod` ([#160](https://github.com/tokio-rs/axum/pull/160))

  With these changes this app using 0.1:

  ```rust
  use axum::{extract::Extension, prelude::*, routing::BoxRoute, AddExtensionLayer};

  let app = route("/", get(|| async { "hi" }))
      .nest("/api", api_routes())
      .layer(AddExtensionLayer::new(state));

  fn api_routes() -> BoxRoute<Body> {
      route(
          "/users",
          post(|Extension(state): Extension<State>| async { "hi from nested" }),
      )
      .boxed()
  }
  ```

  Becomes this in 0.2:

  ```rust
  use axum::{
      extract::Extension,
      handler::{get, post},
      routing::BoxRoute,
      Router,
  };

  let app = Router::new()
      .route("/", get(|| async { "hi" }))
      .nest("/api", api_routes());

  fn api_routes() -> Router<BoxRoute> {
      Router::new()
          .route(
              "/users",
              post(|Extension(state): Extension<State>| async { "hi from nested" }),
          )
          .boxed()
  }
  ```
- Extractors:
  - **added:** Make `FromRequest` default to being generic over `body::Body` ([#146](https://github.com/tokio-rs/axum/pull/146))
  - **added:** Implement `std::error::Error` for all rejections ([#153](https://github.com/tokio-rs/axum/pull/153))
  - **added:** Add `OriginalUri` for extracting original request URI in nested services ([#197](https://github.com/tokio-rs/axum/pull/197))
  - **added:** Implement `FromRequest` for `http::Extensions` ([#169](https://github.com/tokio-rs/axum/pull/169))
  - **added:** Make `RequestParts::{new, try_into_request}` public so extractors can be used outside axum ([#194](https://github.com/tokio-rs/axum/pull/194))
  - **added:** Implement `FromRequest` for `axum::body::Body` ([#241](https://github.com/tokio-rs/axum/pull/241))
  - **changed:** Removed `extract::UrlParams` and `extract::UrlParamsMap`. Use `extract::Path` instead ([#154](https://github.com/tokio-rs/axum/pull/154))
  - **changed:** `extractor_middleware` now requires `RequestBody: Default` ([#167](https://github.com/tokio-rs/axum/pull/167))
  - **changed:** Convert `RequestAlreadyExtracted` to an enum with each possible error variant ([#167](https://github.com/tokio-rs/axum/pull/167))
  - **changed:** `extract::BodyStream` is no longer generic over the request body ([#234](https://github.com/tokio-rs/axum/pull/234))
  - **changed:** `extract::Body` has been renamed to `extract::RawBody` to avoid conflicting with `body::Body` ([#233](https://github.com/tokio-rs/axum/pull/233))
  - **changed:** `RequestParts` changes ([#153](https://github.com/tokio-rs/axum/pull/153))
      - `method` new returns an `&http::Method`
      - `method_mut` new returns an `&mut http::Method`
      - `take_method` has been removed
      - `uri` new returns an `&http::Uri`
      - `uri_mut` new returns an `&mut http::Uri`
      - `take_uri` has been removed
  - **changed:** Remove several rejection types that were no longer used ([#153](https://github.com/tokio-rs/axum/pull/153)) ([#154](https://github.com/tokio-rs/axum/pull/154))
- Responses:
  - **added:** Add `Headers` for easily customizing headers on a response ([#193](https://github.com/tokio-rs/axum/pull/193))
  - **added:** Add `Redirect` response ([#192](https://github.com/tokio-rs/axum/pull/192))
  - **added:** Add `body::StreamBody` for easily responding with a stream of byte chunks ([#237](https://github.com/tokio-rs/axum/pull/237))
  - **changed:** Add associated `Body` and `BodyError` types to `IntoResponse`. This is
    required for returning responses with bodies other than `hyper::Body` from
    handlers. See the docs for advice on how to implement `IntoResponse` ([#86](https://github.com/tokio-rs/axum/pull/86))
  - **changed:** `tower::util::Either` no longer implements `IntoResponse` ([#229](https://github.com/tokio-rs/axum/pull/229))

  This `IntoResponse` from 0.1:
  ```rust
  use axum::{http::Response, prelude::*, response::IntoResponse};

  struct MyResponse;

  impl IntoResponse for MyResponse {
      fn into_response(self) -> Response<Body> {
          Response::new(Body::empty())
      }
  }
  ```

  Becomes this in 0.2:
  ```rust
  use axum::{body::Body, http::Response, response::IntoResponse};

  struct MyResponse;

  impl IntoResponse for MyResponse {
      type Body = Body;
      type BodyError = <Self::Body as axum::body::HttpBody>::Error;

      fn into_response(self) -> Response<Self::Body> {
          Response::new(Body::empty())
      }
  }
  ```
- SSE:
  - **added:** Add `response::sse::Sse`. This implements SSE using a response rather than a service ([#98](https://github.com/tokio-rs/axum/pull/98))
  - **changed:** Remove `axum::sse`. Its been replaced by `axum::response::sse` ([#98](https://github.com/tokio-rs/axum/pull/98))

  Handler using SSE in 0.1:
  ```rust
  use axum::{
      prelude::*,
      sse::{sse, Event},
  };
  use std::convert::Infallible;

  let app = route(
      "/",
      sse(|| async {
          let stream = futures::stream::iter(vec![Ok::<_, Infallible>(
              Event::default().data("hi there!"),
          )]);
          Ok::<_, Infallible>(stream)
      }),
  );
  ```

  Becomes this in 0.2:

  ```rust
  use axum::{
      handler::get,
      response::sse::{Event, Sse},
      Router,
  };
  use std::convert::Infallible;

  let app = Router::new().route(
      "/",
      get(|| async {
          let stream = futures::stream::iter(vec![Ok::<_, Infallible>(
              Event::default().data("hi there!"),
          )]);
          Sse::new(stream)
      }),
  );
  ```
- WebSockets:
  - **changed:** Change WebSocket API to use an extractor plus a response ([#121](https://github.com/tokio-rs/axum/pull/121))
  - **changed:** Make WebSocket `Message` an enum ([#116](https://github.com/tokio-rs/axum/pull/116))
  - **changed:** `WebSocket` now uses `Error` as its error type ([#150](https://github.com/tokio-rs/axum/pull/150))

  Handler using WebSockets in 0.1:

  ```rust
  use axum::{
      prelude::*,
      ws::{ws, WebSocket},
  };

  let app = route(
      "/",
      ws(|socket: WebSocket| async move {
          // do stuff with socket
      }),
  );
  ```

  Becomes this in 0.2:

  ```rust
  use axum::{
      extract::ws::{WebSocket, WebSocketUpgrade},
      handler::get,
      Router,
  };

  let app = Router::new().route(
      "/",
      get(|ws: WebSocketUpgrade| async move {
          ws.on_upgrade(|socket: WebSocket| async move {
              // do stuff with socket
          })
      }),
  );
  ```
- Misc
  - **added:** Add default feature `tower-log` which exposes `tower`'s `log` feature. ([#218](https://github.com/tokio-rs/axum/pull/218))
  - **changed:** Replace `body::BoxStdError` with `axum::Error`, which supports downcasting ([#150](https://github.com/tokio-rs/axum/pull/150))
  - **changed:** `EmptyRouter` now requires the response body to implement `Send + Sync + 'static'` ([#108](https://github.com/tokio-rs/axum/pull/108))
  - **changed:** `Router::check_infallible` now returns a `CheckInfallible` service. This
    is to improve compile times ([#198](https://github.com/tokio-rs/axum/pull/198))
  - **changed:** `Router::into_make_service` now returns `routing::IntoMakeService` rather than
    `tower::make::Shared` ([#229](https://github.com/tokio-rs/axum/pull/229))
  - **changed:** All usage of `tower::BoxError` has been replaced with `axum::BoxError` ([#229](https://github.com/tokio-rs/axum/pull/229))
  - **changed:** Several response future types have been moved into dedicated
    `future` modules ([#133](https://github.com/tokio-rs/axum/pull/133))
  - **changed:** `EmptyRouter`, `ExtractorMiddleware`, `ExtractorMiddlewareLayer`,
    and `QueryStringMissing` no longer implement `Copy` ([#132](https://github.com/tokio-rs/axum/pull/132))
  - **changed:** `service::OnMethod`, `handler::OnMethod`, and `routing::Nested` have new response future types ([#157](https://github.com/tokio-rs/axum/pull/157))

# 0.1.3 (06. August, 2021)

- Fix stripping prefix when nesting services at `/` ([#91](https://github.com/tokio-rs/axum/pull/91))
- Add support for WebSocket protocol negotiation ([#83](https://github.com/tokio-rs/axum/pull/83))
- Use `pin-project-lite` instead of `pin-project` ([#95](https://github.com/tokio-rs/axum/pull/95))
- Re-export `http` crate and `hyper::Server` ([#110](https://github.com/tokio-rs/axum/pull/110))
- Fix `Query` and `Form` extractors giving bad request error when query string is empty. ([#117](https://github.com/tokio-rs/axum/pull/117))
- Add `Path` extractor. ([#124](https://github.com/tokio-rs/axum/pull/124))
- Fixed the implementation of `IntoResponse` of `(HeaderMap, T)` and `(StatusCode, HeaderMap, T)` would ignore headers from `T` ([#137](https://github.com/tokio-rs/axum/pull/137))
- Deprecate `extract::UrlParams` and `extract::UrlParamsMap`. Use `extract::Path` instead ([#138](https://github.com/tokio-rs/axum/pull/138))

# 0.1.2 (01. August, 2021)

- Implement `Stream` for `WebSocket` ([#52](https://github.com/tokio-rs/axum/pull/52))
- Implement `Sink` for `WebSocket` ([#52](https://github.com/tokio-rs/axum/pull/52))
- Implement `Deref` most extractors ([#56](https://github.com/tokio-rs/axum/pull/56))
- Return `405 Method Not Allowed` for unsupported method for route ([#63](https://github.com/tokio-rs/axum/pull/63))
- Add extractor for remote connection info ([#55](https://github.com/tokio-rs/axum/pull/55))
- Improve error message of `MissingExtension` rejections ([#72](https://github.com/tokio-rs/axum/pull/72))
- Improve documentation for routing ([#71](https://github.com/tokio-rs/axum/pull/71))
- Clarify required response body type when routing to `tower::Service`s ([#69](https://github.com/tokio-rs/axum/pull/69))
- Add `axum::body::box_body` to converting an `http_body::Body` to `axum::body::BoxBody` ([#69](https://github.com/tokio-rs/axum/pull/69))
- Add `axum::sse` for Server-Sent Events ([#75](https://github.com/tokio-rs/axum/pull/75))
- Mention required dependencies in docs ([#77](https://github.com/tokio-rs/axum/pull/77))
- Fix WebSockets failing on Firefox ([#76](https://github.com/tokio-rs/axum/pull/76))

# 0.1.1 (30. July, 2021)

- Misc readme fixes.

# 0.1.0 (30. July, 2021)

- Initial release.
