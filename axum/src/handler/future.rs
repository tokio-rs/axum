//! Handler future types.

use crate::body::BoxBody;
use futures_util::future::{BoxFuture, Map};
use http::Response;
use std::convert::Infallible;

opaque_future! {
    /// The response future for [`IntoService`](super::IntoService).
    pub type IntoServiceFuture =
        Map<
            BoxFuture<'static, Response<BoxBody>>,
            fn(Response<BoxBody>) -> Result<Response<BoxBody>, Infallible>,
        >;
}
