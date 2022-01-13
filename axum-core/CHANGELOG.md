# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Unreleased

- **breaking:** Using `HeaderMap` as an extractor will no longer remove the headers and thus
  they'll still be accessible to other extractors, such as `axum::extract::Json`. Instead
  `HeaderMap` will clone the headers. You should prefer to use `TypedHeader` to extract only the
  headers you need ([#698])

  This includes these breaking changes:
    - `RequestParts::take_headers` has been removed.
    - `RequestParts::headers` returns `&HeaderMap`.
    - `RequestParts::headers_mut` returns `&mut HeaderMap`.
    - `HeadersAlreadyExtracted` has been removed.
    - The `HeadersAlreadyExtracted` variant has been removed from these rejections:
        - `RequestAlreadyExtracted`
        - `RequestPartsAlreadyExtracted`
    - `<HeaderMap as FromRequest<_>>::Error` has been changed to `std::convert::Infallible`.
- **breaking:** `axum::http::Extensions` is no longer an extractor (ie it
  doesn't implement `FromRequest`). The `axum::extract::Extension` extractor is
  _not_ impacted by this and works the same. This change makes it harder to
  accidentally remove all extensions which would result in confusing errors
  elsewhere ([#699])
  This includes these breaking changes:
    - `RequestParts::take_extensions` has been removed.
    - `RequestParts::extensions` returns `&Extensions`.
    - `RequestParts::extensions_mut` returns `&mut Extensions`.
    - `RequestAlreadyExtracted` has been removed.
    - `<Request as FromRequest>::Error` is now `BodyAlreadyExtracted`.
    - `<http::request::Parts as FromRequest>::Error` is now `Infallible`.
    - `ExtensionsAlreadyExtracted` has been removed.

[#698]: https://github.com/tokio-rs/axum/pull/698
[#699]: https://github.com/tokio-rs/axum/pull/699

# 0.1.1 (06. December, 2021)

- **added:** `axum_core::response::Response` now exists as a shorthand for writing `Response<BoxBody>` ([#590])

[#590]: https://github.com/tokio-rs/axum/pull/590

# 0.1.0 (02. December, 2021)

- Initial release.
