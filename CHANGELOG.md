# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Unreleased

- Make `FromRequest` default to being generic over `axum::body::Body` ([#146](https://github.com/tokio-rs/axum/pull/146))
- Implement `std::error::Error` for all rejections ([#153](https://github.com/tokio-rs/axum/pull/153))
- Fix `Uri` extractor not being the full URI if using `nest` ([#156](https://github.com/tokio-rs/axum/pull/156))

## Breaking changes

- Add associated `Body` and `BodyError` types to `IntoResponse`. This is
  required for returning responses with bodies other than `hyper::Body` from
  handlers. See the docs for advice on how to implement `IntoResponse` ([#86](https://github.com/tokio-rs/axum/pull/86))
- Change WebSocket API to use an extractor ([#121](https://github.com/tokio-rs/axum/pull/121))
- Make WebSocket `Message` an enum ([#116](https://github.com/tokio-rs/axum/pull/116))
- Add `RoutingDsl::or` for combining routes. ([#108](https://github.com/tokio-rs/axum/pull/108))
- Ensure a `HandleError` service created from `axum::ServiceExt::handle_error`
  _does not_ implement `RoutingDsl` as that could lead to confusing routing
  behavior. ([#120](https://github.com/tokio-rs/axum/pull/120))
- Remove `QueryStringMissing` as it was no longer being used
- `extract::extractor_middleware::ExtractorMiddlewareResponseFuture` moved
  to `extract::extractor_middleware::future::ResponseFuture` ([#133](https://github.com/tokio-rs/axum/pull/133))
- `routing::BoxRouteFuture` moved to `routing::future::BoxRouteFuture` ([#133](https://github.com/tokio-rs/axum/pull/133))
- `routing::EmptyRouterFuture` moved to `routing::future::EmptyRouterFuture` ([#133](https://github.com/tokio-rs/axum/pull/133))
- `routing::RouteFuture` moved to `routing::future::RouteFuture` ([#133](https://github.com/tokio-rs/axum/pull/133))
- `service::BoxResponseBodyFuture` moved to `service::future::BoxResponseBodyFuture` ([#133](https://github.com/tokio-rs/axum/pull/133))
- The following types no longer implement `Copy` ([#132](https://github.com/tokio-rs/axum/pull/132))
    - `EmptyRouter`
    - `ExtractorMiddleware`
    - `ExtractorMiddlewareLayer`
- Replace `axum::body::BoxStdError` with `axum::Error`, which supports downcasting ([#150](https://github.com/tokio-rs/axum/pull/150))
- `WebSocket` now uses `axum::Error` as its error type ([#150](https://github.com/tokio-rs/axum/pull/150))
- `RequestParts` changes ([#153](https://github.com/tokio-rs/axum/pull/153))
    - `method` new returns an `&http::Method`
    - `method_mut` new returns an `&mut http::Method`
    - `take_method` has been removed
    - `uri` new returns an `&http::Uri`
    - `uri_mut` new returns an `&mut http::Uri`
    - `take_uri` has been removed
- These rejections have been removed as they're no longer used
    - `MethodAlreadyExtracted` ([#153](https://github.com/tokio-rs/axum/pull/153))
    - `UriAlreadyExtracted` ([#153](https://github.com/tokio-rs/axum/pull/153))
    - `VersionAlreadyExtracted` ([#153](https://github.com/tokio-rs/axum/pull/153))
    - `UrlParamsRejection`
    - `InvalidUrlParam`
- Removed `extract::UrlParams` and `extract::UrlParamsMap`. Use `extract::Path` instead
- The following services have new response future types:
  - `service::OnMethod`
  - `handler::OnMethod`
  - `routing::Nested`

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
