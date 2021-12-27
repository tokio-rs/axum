//! Additional types for creating middleware.

pub mod middleware_fn;

pub use self::middleware_fn::{from_fn, Next};
