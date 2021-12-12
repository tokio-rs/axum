# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Unreleased

- None.

# 0.3.2 (09. December 2021)

- Support checking `FromRequest` bounds for extractors whose request body is something else than
  `axum::body::Body`. Use `#[debug_handler(body = YourBodyType)]` to use a different request body
  type ([#595])

[#595]: https://github.com/tokio-rs/axum/pull/595

# 0.3.1 (06. December 2021)

- Fix `Result<impl IntoResponse, Error>` generating invalid code ([#588])

[#588]: https://github.com/tokio-rs/axum/pull/588

# 0.3.0 (03. December 2021)

- Update to axum 0.4. axum-debug will _not_ work with axum 0.3.x.

# 0.2.2 (22. October 2021)

- Fix regression causing errors when `#[debug_handler]` was used on functions with multiple
  extractors ([#552])

[#552]: https://github.com/tokio-rs/axum/pull/552

# 0.2.1 (19. October 2021)

- Make macro handle more cases such as mutable extractors and handlers taking
  `self` ([#518])

[#518]: https://github.com/tokio-rs/axum/pull/518

# 0.2.0 (13. October 2021)

- **breaking:** Removed `debug_router` macro.
- **breaking:** Removed `check_service` function.
- **breaking:** Removed `debug_service` function.

# 0.1.0 (6. October 2021)

- Initial release.
