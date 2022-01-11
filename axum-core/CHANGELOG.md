# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Unreleased

- **breaking:** Using `HeaderMap` as an extractor will no longer remove the headers and thus
  they'll still be accessible to other extractors, such as `axum::extract::Json`. Instead
  `HeaderMap` will clone the request. You should prefer to use `TypedHeader` to extract only the
  header you need ([#698])

  This includes these breaking changes:
    - `RequestParts::take_headers` has been removed.
    - `RequestParts::headers` returns `&HeaderMap`.
    - `RequestParts::headers_mut` returns `&mut HeaderMap`.
    - `HeadersAlreadyExtracted` has been removed.
    - The `HeadersAlreadyExtracted` removed variant has been removed from these rejections:
        - `RequestAlreadyExtracted`
        - `RequestPartsAlreadyExtracted`
        - `JsonRejection`
        - `FormRejection`
        - `ContentLengthLimitRejection`
        - `WebSocketUpgradeRejection`
    - `<HeaderMap as FromRequest<_>>::Error` has been changed to `std::convert::Infallible`.

[#698]: https://github.com/tokio-rs/axum/pull/698

# 0.1.1 (06. December, 2021)

- **added:** `axum_core::response::Response` now exists as a shorthand for writing `Response<BoxBody>` ([#590])

[#590]: https://github.com/tokio-rs/axum/pull/590

# 0.1.0 (02. December, 2021)

- Initial release.
