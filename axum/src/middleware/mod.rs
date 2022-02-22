//! Utilities for writing middleware

mod from_fn;

pub use self::from_fn::{from_fn, FromFn, FromFnLayer, Next};

pub mod futures {
    //! Future types.

    pub use super::from_fn::ResponseFuture as FromFnResponseFuture;
}
