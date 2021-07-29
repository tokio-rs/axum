//! Future types.

use crate::body::BoxBody;
use http::Response;
use std::convert::Infallible;

opaque_future! {
    /// Response future for [`WebSocketUpgrade`](super::WebSocketUpgrade).
    pub type ResponseFuture = futures_util::future::BoxFuture<'static, Result<Response<BoxBody>, Infallible>>;
}
