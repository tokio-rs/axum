// #![doc(html_root_url = "https://docs.rs/tower-http/0.1.0")]
#![warn(
    clippy::all,
    clippy::dbg_macro,
    clippy::todo,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::pub_enum_variant_names,
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
    // missing_docs,
)]
#![deny(unreachable_pub, broken_intra_doc_links, private_in_public)]
#![allow(
    elided_lifetimes_in_paths,
    // TODO: Remove this once the MSRV bumps to 1.42.0 or above.
    clippy::match_like_matches_macro,
    clippy::type_complexity
)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]

use self::body::Body;
use bytes::Bytes;
use http::{Request, Response};
use response::IntoResponse;
use routing::{EmptyRouter, Route};
use std::convert::Infallible;
use tower::{BoxError, Service};

pub mod body;
pub mod extract;
pub mod handler;
pub mod response;
pub mod routing;
pub mod service;

#[doc(inline)]
pub use self::{
    handler::{get, on, post, Handler},
    routing::AddRoute,
};

pub use async_trait::async_trait;
pub use tower_http::add_extension::{AddExtension, AddExtensionLayer};

pub fn route<S>(spec: &str, svc: S) -> Route<S, EmptyRouter>
where
    S: Service<Request<Body>, Error = Infallible> + Clone,
{
    routing::EmptyRouter.route(spec, svc)
}

#[cfg(test)]
mod tests;

pub(crate) trait ResultExt<T> {
    fn unwrap_infallible(self) -> T;
}

impl<T> ResultExt<T> for Result<T, Infallible> {
    fn unwrap_infallible(self) -> T {
        match self {
            Ok(value) => value,
            Err(err) => match err {},
        }
    }
}

pub trait ServiceExt<B>: Service<Request<Body>, Response = Response<B>> {
    fn handle_error<F, Res>(self, f: F) -> service::HandleError<Self, F>
    where
        Self: Sized,
        F: FnOnce(Self::Error) -> Res,
        Res: IntoResponse<Body>,
        B: http_body::Body<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
    {
        service::HandleError::new(self, f)
    }
}

impl<S, B> ServiceExt<B> for S where S: Service<Request<Body>, Response = Response<B>> {}
