//! Handler future types.

use crate::body::BoxBody;
use http::Response;
use std::convert::Infallible;

opaque_future! {
    /// The response future for [`IntoService`](super::IntoService).
    pub type IntoServiceFuture =
        futures_util::future::BoxFuture<'static, Result<Response<BoxBody>, Infallible>>;
}
