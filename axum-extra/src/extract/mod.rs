//! Additional extractors.

mod cached;

pub use self::cached::Cached;

pub mod rejection {
    //! Rejection response types.

    pub use super::cached::CachedRejection;
}
