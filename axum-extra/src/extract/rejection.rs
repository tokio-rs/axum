use axum_core::__composite_rejection as composite_rejection;
use axum_core::__define_rejection as define_rejection;

use axum_core::extract::rejection::*;

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to deserialize query string"]
    /// Rejection type used if the [`Query`](super::Query) extractor is unable to
    /// deserialize the query string into the target type.
    pub struct FailedToDeserializeQueryString(Error);
}

composite_rejection! {
    /// Rejection used for [`Query`](super::Query).
    ///
    /// Contains one variant for each way the [`Query`](super::Query) extractor
    /// can fail.
    pub enum QueryRejection {
        FailedToDeserializeQueryString,
    }
}
