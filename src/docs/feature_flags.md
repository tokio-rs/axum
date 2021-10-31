# Feature flags

axum uses a set of [feature flags] to reduce the amount of compiled and
optional dependencies.

The following optional features are available:

- `headers`: Enables extracting typed headers via [`extract::TypedHeader`].
- `http1`: Enables hyper's `http1` feature. Enabled by default.
- `http2`: Enables hyper's `http2` feature.
- `json`: Enables the [`Json`] type and some similar convenience functionality.
  Enabled by default.
- `multipart`: Enables parsing `multipart/form-data` requests with [`extract::Multipart`].
- `tower-log`: Enables `tower`'s `log` feature. Enabled by default.
- `ws`: Enables WebSockets support via [`extract::ws`].
