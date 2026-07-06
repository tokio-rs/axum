# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Unreleased

- Initial release. `axum-serve` provides the `serve` function and the `Listener`
  trait that power `axum::serve`, extracted from `axum` so that libraries can
  implement a custom `Listener` (for example, a TLS-terminating listener) without
  depending on all of `axum` ([#3433]).

[#3433]: https://github.com/tokio-rs/axum/issues/3433
