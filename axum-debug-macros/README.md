[![License](https://img.shields.io/crates/l/axum-debug-macros)](https://choosealicense.com/licenses/mit/)
[![Crates.io](https://img.shields.io/crates/v/axum-debug-macros)](https://crates.io/crates/axum-debug-macros)
[![Docs - Stable](https://img.shields.io/crates/v/axum-debug-macros?color=blue&label=docs)](https://docs.rs/axum-debug-macros/)

# axum-debug-macros

Procedural macros for [`axum-debug`] crate.

## Safety

This crate uses `#![forbid(unsafe_code)]` to ensure everything is implemented
in 100% safe Rust.

## Performance

This crate have no effect when using release profile. (eg. `cargo build
--release`)

## License

This project is licensed under the [MIT license](LICENSE).

[`axum-debug`]: https://crates.io/crates/axum-debug
