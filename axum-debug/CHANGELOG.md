# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Unreleased

- None.

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
