//! Handler future types.

use crate::response::Response;
use futures_util::future::{BoxFuture, Map};
use std::convert::Infallible;

opaque_future! {
    /// The response future for [`IntoService`](super::IntoService).
    pub type IntoServiceFuture =
        Map<
            BoxFuture<'static, Response>,
            fn(Response) -> Result<Response, Infallible>,
        >;
}
