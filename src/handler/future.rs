//! Handler future types.

use std::convert::Infallible;

use http::Response;

use crate::body::BoxBody;

opaque_future! {
    /// The response future for [`IntoService`](super::IntoService).
    pub type IntoServiceFuture =
        futures_util::future::BoxFuture<'static, Result<Response<BoxBody>, Infallible>>;
}
