//! Additional extractors.

mod cached;
mod or_default;

pub use self::{cached::Cached, or_default::OrDefault};

pub mod rejection {
    //! Rejection response types.

    pub use super::cached::CachedRejection;
}
