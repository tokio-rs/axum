//! Future types.

use crate::body::BoxBody;
use futures_util::future::Either;
use http::Response;
use std::{convert::Infallible, future::ready};

pub use super::{into_make_service::IntoMakeServiceFuture, route::RouteFuture};

opaque_future! {
    /// Response future for [`Router`](super::Router).
    pub type RouterFuture<B> =
        futures_util::future::Either<
            RouteFuture<B, Infallible>,
            std::future::Ready<Result<Response<BoxBody>, Infallible>>,
        >;
}

impl<B> RouterFuture<B> {
    pub(super) fn from_future(future: RouteFuture<B, Infallible>) -> Self {
        Self::new(Either::Left(future))
    }

    pub(super) fn from_response(response: Response<BoxBody>) -> Self {
        Self::new(Either::Right(ready(Ok(response))))
    }
}
