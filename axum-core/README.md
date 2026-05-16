# axum-core

[![Build status](https://github.com/tokio-rs/axum/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/tokio-rs/axum-core/actions/workflows/CI.yml)
[![Crates.io](https://img.shields.io/crates/v/axum-core)](https://crates.io/crates/axum-core)
[![Documentation](https://docs.rs/axum-core/badge.svg)](https://docs.rs/axum-core)

Core types and traits for axum.

More information about this crate can be found in the [crate documentation][docs].

## Safety

`unsafe_code` is denied by default. Any `unsafe` must be opted in explicitly
with `#[allow(unsafe_code)]` and must be accompanied by a `SAFETY` comment
(enforced by `clippy::undocumented_unsafe_blocks`).

## Minimum supported Rust version

axum-core's MSRV is 1.75.

## Getting Help

You're also welcome to ask in the [Discord channel][chat] or open an [issue]
with your question.

## Contributing

🎈 Thanks for your help improving the project! We are so happy to have
you! We have a [contributing guide][contributing] to help you get involved in the
`axum` project.

## License

This project is licensed under the [MIT license][license].

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `axum` by you, shall be licensed as MIT, without any
additional terms or conditions.

[`axum`]: https://crates.io/crates/axum
[chat]: https://discord.gg/tokio
[contributing]: /CONTRIBUTING.md
[docs]: https://docs.rs/axum-core
[license]: /axum-core/LICENSE
[issue]: https://github.com/tokio-rs/axum/issues/new
