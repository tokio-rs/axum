# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Unreleased

- **added:** Add `#[derive(TypedPath)]` for use with axum-extra's new "type safe" routing API ([#756])
- **added:** `#[derive(TypedPath)]` now also generates a `TryFrom<_> for Uri`
  implementation ([#790])

[#790]: https://github.com/tokio-rs/axum/pull/790

# 0.1.0 (31. January, 2022)

- Initial release.

[#756]: https://github.com/tokio-rs/axum/pull/756
