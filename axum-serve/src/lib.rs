//! Serve `axum` (and other [`tower`]-based) services with [`hyper`].
//!
//! This crate provides the [`serve`] function and the [`Listener`] trait that power
//! `axum::serve`. It intentionally does not depend on the `axum` crate, so libraries that
//! want to implement a custom [`Listener`] (for example, a TLS-terminating listener) can
//! depend on `axum-serve` directly instead of pulling in all of `axum`.
//!
//! Most users should not depend on this crate directly and should instead use the
//! re-exports available at `axum::serve`.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(not(test), warn(clippy::print_stdout, clippy::dbg_macro))]

#[macro_use]
mod macros;

mod listener;
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
mod serve;

pub use self::listener::{ConnLimiter, ConnLimiterIo, Listener, ListenerExt, TapIo};
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
pub use self::serve::{
    serve, ConnectionLifetimeLimits, Executor, IncomingStream, MaxConnectionAge, Serve,
    TokioExecutor, WithGracefulShutdown,
};
