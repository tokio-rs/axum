//! Handler future types.

use crate::response::Response;
use futures_util::future::Map;
use std::convert::Infallible;

opaque_future! {
    /// The response future for [`IntoService`](super::IntoService).
    pub type IntoServiceFuture<F> =
        Map<
            F,
            fn(Response) -> Result<Response, Infallible>,
        >;
}
