use std::ops::Deref;

pub use either::Either;

use axum::handler::Handler;
use axum::http::Request;
use axum::Json;

#[macro_use]
mod macros;

pub mod openapi;

pub mod routing;

pub mod handler;

