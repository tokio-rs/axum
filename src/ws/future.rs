//! Future types.

use std::convert::Infallible;

use http::Response;

use crate::body::BoxBody;

opaque_future! {
    /// Response future for [`WebSocketUpgrade`](super::WebSocketUpgrade).
    pub type ResponseFuture = futures_util::future::BoxFuture<'static, Result<Response<BoxBody>, Infallible>>;
}
