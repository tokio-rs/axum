//! Rejection response types.

use axum_core::{
    __composite_rejection as composite_rejection, __define_rejection as define_rejection,
};

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "No host found in request"]
    /// Rejection type used if the [`Host`](super::Host) extractor is unable to
    /// resolve a host.
    pub struct FailedToResolveHost;
}

composite_rejection! {
    /// Rejection used for [`Host`](super::Host).
    ///
    /// Contains one variant for each way the [`Host`](super::Host) extractor
    /// can fail.
    pub enum HostRejection {
        FailedToResolveHost,
    }
}
