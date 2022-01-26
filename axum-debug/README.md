# axum-debug

[![Build status](https://github.com/tokio-rs/axum/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/tokio-rs/axum-debug/actions/workflows/CI.yml)
[![Crates.io](https://img.shields.io/crates/v/axum-debug)](https://crates.io/crates/axum-debug)
[![Documentation](https://docs.rs/axum-debug/badge.svg)](https://docs.rs/axum-debug)

This is a debugging crate that provides better error messages for [`axum`]
framework.

**Note:** this crate is deprecated. Use [axum-macros] instead.

More information about this crate can be found in the [crate documentation][docs].

## Safety

This crate uses `#![forbid(unsafe_code)]` to ensure everything is implemented in 100% safe Rust.

## Minimum supported Rust version

axum-debug's MSRV is 1.54.

## Getting Help

You're also welcome to ask in the [Discord channel][chat] or open an [issue]
with your question.

## Contributing

:balloon: Thanks for your help improving the project! We are so happy to have
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
[docs]: https://docs.rs/axum-debug
[license]: /axum-debug/LICENSE
[issue]: https://github.com/tokio-rs/axum/issues/new
[axum-macros]: https://crates.io/crates/axum-macros
