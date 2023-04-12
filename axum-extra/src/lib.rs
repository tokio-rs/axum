//! Extra utilities for [`axum`].
//!
//! # Feature flags
//!
//! axum-extra uses a set of [feature flags] to reduce the amount of compiled and
//! optional dependencies.
//!
//! The following optional features are available:
//!
//! Name | Description | Default?
//! ---|---|---
//! `async-read-body` | Enables the `AsyncReadBody` body | No
//! `cookie` | Enables the `CookieJar` extractor | No
//! `cookie-private` | Enables the `PrivateCookieJar` extractor | No
//! `cookie-signed` | Enables the `SignedCookieJar` extractor | No
//! `cookie-key-expansion` | Enables the `Key::derive_from` method | No
//! `erased-json` | Enables the `ErasedJson` response | No
//! `form` | Enables the `Form` extractor | No
//! `json-lines` | Enables the `JsonLines` extractor and response | No
//! `multipart` | Enables the `Multpart` extractor | No
//! `protobuf` | Enables the `Protobuf` extractor and response | No
//! `query` | Enables the `Query` extractor | No
//! `typed-routing` | Enables the `TypedPath` routing utilities | No
//!
//! [`axum`]: https://crates.io/crates/axum

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
    clippy::str_to_string,
    rust_2018_idioms,
    future_incompatible,
    nonstandard_style,
    missing_debug_implementations,
    missing_docs
)]
#![deny(unreachable_pub, private_in_public)]
#![allow(elided_lifetimes_in_paths, clippy::type_complexity)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(not(test), warn(clippy::print_stdout, clippy::dbg_macro))]

#[allow(unused_extern_crates)]
extern crate self as axum_extra;

pub mod body;
pub mod either;
pub mod extract;
pub mod handler;
pub mod middleware;
pub mod response;
pub mod routing;

#[cfg(feature = "json-lines")]
pub mod json_lines;

#[cfg(feature = "protobuf")]
pub mod protobuf;

#[cfg(feature = "typed-routing")]
#[doc(hidden)]
pub mod __private {
    //! _not_ public API

    use percent_encoding::{AsciiSet, CONTROLS};

    pub use percent_encoding::utf8_percent_encode;

    // from https://github.com/servo/rust-url/blob/master/url/src/parser.rs
    const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
    const PATH: &AsciiSet = &FRAGMENT.add(b'#').add(b'?').add(b'{').add(b'}');
    pub const PATH_SEGMENT: &AsciiSet = &PATH.add(b'/').add(b'%');
}

#[cfg(test)]
use axum_macros::__private_axum_test as test;

#[cfg(test)]
pub(crate) mod test_helpers {
    #![allow(unused_imports)]

    use axum::{body::HttpBody, BoxError, Router};

    mod test_client {
        #![allow(dead_code)]
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../axum/src/test_helpers/test_client.rs"
        ));
    }
    pub(crate) use self::test_client::*;
}
