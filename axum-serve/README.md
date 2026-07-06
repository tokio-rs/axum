# axum-serve

Serve axum (and other [`tower`](https://crates.io/crates/tower)-based) services with
[hyper](https://crates.io/crates/hyper).

This crate provides the `serve` function and the `Listener` trait that power `axum::serve`.
It has no dependency on the `axum` crate itself, so libraries that want to implement a custom
[`Listener`] (for example, a TLS-terminating listener) can depend on `axum-serve` directly
instead of pulling in all of `axum`.

Most users should not depend on this crate directly and should instead use the re-exports
available at `axum::serve`.

More information about axum can be found in the [crate docs](https://docs.rs/axum).

[`Listener`]: https://docs.rs/axum-serve/latest/axum_serve/trait.Listener.html
