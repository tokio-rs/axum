//! axum is a web application framework that focuses on ergonomics and modularity.
//!
#![doc = include_str!("docs/table_of_contents.md")]
#![doc = include_str!("docs/high_level_features.md")]
#![doc = include_str!("docs/compatibility.md")]
#![doc = include_str!("docs/example.md")]
#![doc = include_str!("docs/handlers.md")]
#![doc = include_str!("docs/routing.md")]
#![doc = include_str!("docs/extractors.md")]
#![doc = include_str!("docs/building_responses.md")]
#![doc = include_str!("docs/error_handling.md")]
#![doc = include_str!("docs/applying_middleware.md")]
#![doc = include_str!("docs/sharing_state_with_handlers.md")]
#![doc = include_str!("docs/required_dependencies.md")]
#![doc = include_str!("docs/examples.md")]
#![doc = include_str!("docs/feature_flags.md")]
//!
//! [`tower`]: https://crates.io/crates/tower
//! [`tower-http`]: https://crates.io/crates/tower-http
//! [`tokio`]: http://crates.io/crates/tokio
//! [`hyper`]: http://crates.io/crates/hyper
//! [`tonic`]: http://crates.io/crates/tonic
//! [feature flags]: https://doc.rust-lang.org/cargo/reference/features.html#the-features-section
//! [`IntoResponse`]: crate::response::IntoResponse
//! [`Timeout`]: tower::timeout::Timeout
//! [examples]: https://github.com/tokio-rs/axum/tree/main/examples
//! [`Router::merge`]: crate::routing::Router::merge
//! [`axum::Server`]: hyper::server::Server
//! [`OriginalUri`]: crate::extract::OriginalUri
//! [`Service`]: tower::Service
//! [`Service::poll_ready`]: tower::Service::poll_ready
//! [`tower::Service`]: tower::Service
//! [`handle_error`]: error_handling::HandleErrorExt::handle_error
//! [tower-guides]: https://github.com/tower-rs/tower/tree/master/guides
//! [`Uuid`]: https://docs.rs/uuid/latest/uuid/
//! [`FromRequest`]: crate::extract::FromRequest
//! [`HeaderMap`]: http::header::HeaderMap
//! [`Request`]: http::Request
//! [customize-extractor-error]: https://github.com/tokio-rs/axum/blob/main/examples/customize-extractor-error/src/main.rs
//! [axum-debug]: https://docs.rs/axum-debug
//! [`debug_handler`]: https://docs.rs/axum-debug/latest/axum_debug/attr.debug_handler.html
//! [`Handler`]: crate::handler::Handler
//! [`Infallible`]: std::convert::Infallible

#![warn(
    clippy::all,
    clippy::dbg_macro,
    clippy::todo,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::mem_forget,
    clippy::unused_self,
    clippy::filter_map_next,
    clippy::needless_continue,
    clippy::needless_borrow,
    clippy::match_wildcard_for_single_variants,
    clippy::if_let_mutex,
    clippy::mismatched_target_os,
    clippy::await_holding_lock,
    clippy::match_on_vec_items,
    clippy::imprecise_flops,
    clippy::suboptimal_flops,
    clippy::lossy_float_literal,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::fn_params_excessive_bools,
    clippy::exit,
    clippy::inefficient_to_string,
    clippy::linkedlist,
    clippy::macro_use_imports,
    clippy::option_option,
    clippy::verbose_file_reads,
    clippy::unnested_or_patterns,
    rust_2018_idioms,
    future_incompatible,
    nonstandard_style,
    missing_debug_implementations,
    missing_docs
)]
#![deny(unreachable_pub, private_in_public)]
#![allow(elided_lifetimes_in_paths, clippy::type_complexity)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]

#[macro_use]
pub(crate) mod macros;

mod add_extension;
mod clone_box_service;
mod error;
#[cfg(feature = "json")]
mod json;
mod util;

pub mod body;
pub mod error_handling;
pub mod extract;
pub mod handler;
pub mod response;
pub mod routing;

#[cfg(test)]
mod tests;

pub use add_extension::{AddExtension, AddExtensionLayer};
#[doc(no_inline)]
pub use async_trait::async_trait;
#[doc(no_inline)]
pub use http;
#[doc(no_inline)]
pub use hyper::Server;

#[doc(inline)]
#[cfg(feature = "json")]
pub use self::json::Json;
#[doc(inline)]
pub use self::{error::Error, routing::Router};

/// Alias for a type-erased error type.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
