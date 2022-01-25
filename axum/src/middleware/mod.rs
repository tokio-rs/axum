//! Utilities for writing middleware

mod from_fn;

pub use self::from_fn::{from_fn, FromFn, FromFnLayer, Next};
