# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Unreleased

- None.

# 0.2.0 (31. March, 2022)

- **breaking:** Routes are now required to start with `/`. Previously empty routes or routes such
  as `:foo` would be accepted but most likely result in bugs ([#823])

[#823]: https://github.com/tokio-rs/axum/pull/823

# 0.1.2 (1. March 2022)

- **fixed:** Use fully qualified `Result` type ([#796])

[#796]: https://github.com/tokio-rs/axum/pull/796

# 0.1.1 (22. February 2022)

- Add `#[derive(TypedPath)]` for use with axum-extra's new "type safe" routing API ([#756])

[#756]: https://github.com/tokio-rs/axum/pull/756

# 0.1.0 (31. January, 2022)

- Initial release.
