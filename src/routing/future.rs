//! Future types.

use crate::body::BoxBody;
use futures_util::future::Either;
use http::{Request, Response};
use std::{convert::Infallible, future::ready};
use tower::util::Oneshot;

pub use super::{
    into_make_service::IntoMakeServiceFuture, method_not_allowed::MethodNotAllowedFuture,
    route::RouteFuture,
};

opaque_future! {
    /// Response future for [`Router`](super::Router).
    pub type RouterFuture<B> =
        futures_util::future::Either<
            Oneshot<super::Route<B>, Request<B>>,
            std::future::Ready<Result<Response<BoxBody>, Infallible>>,
        >;
}

impl<B> RouterFuture<B> {
    pub(super) fn from_oneshot(future: Oneshot<super::Route<B>, Request<B>>) -> Self {
        Self::new(Either::Left(future))
    }

    pub(super) fn from_response(response: Response<BoxBody>) -> Self {
        RouterFuture::new(Either::Right(ready(Ok(response))))
    }
}
