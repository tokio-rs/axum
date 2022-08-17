use super::HandlerCallWithExtractors;
use crate::either::Either;
use axum::{
    extract::{FromRequest, RequestParts},
    handler::Handler,
    http::Request,
    response::{IntoResponse, Response},
};
use futures_util::future::{BoxFuture, Either as EitherFuture, FutureExt, Map};
use http::StatusCode;
use std::{future::Future, marker::PhantomData};

/// [`Handler`] that runs one [`Handler`] and if that rejects it'll fallback to another
/// [`Handler`].
///
/// Created with [`HandlerCallWithExtractors::or`](super::HandlerCallWithExtractors::or).
#[allow(missing_debug_implementations)]
pub struct Or<L, R, Lt, Rt, B> {
    pub(super) lhs: L,
    pub(super) rhs: R,
    pub(super) _marker: PhantomData<fn() -> (Lt, Rt, B)>,
}

impl<B, L, R, Lt, Rt> HandlerCallWithExtractors<Either<Lt, Rt>, B> for Or<L, R, Lt, Rt, B>
where
    L: HandlerCallWithExtractors<Lt, B> + Send + 'static,
    R: HandlerCallWithExtractors<Rt, B> + Send + 'static,
    Rt: Send + 'static,
    Lt: Send + 'static,
    B: Send + 'static,
{
    // this puts `futures_util` in our public API but thats fine in axum-extra
    type Future = EitherFuture<
        Map<L::Future, fn(<L::Future as Future>::Output) -> Response>,
        Map<R::Future, fn(<R::Future as Future>::Output) -> Response>,
    >;

    fn call(
        self,
        extractors: Either<Lt, Rt>,
    ) -> <Self as HandlerCallWithExtractors<Either<Lt, Rt>, B>>::Future {
        match extractors {
            Either::E1(lt) => self
                .lhs
                .call(lt)
                .map(IntoResponse::into_response as _)
                .left_future(),
            Either::E2(rt) => self
                .rhs
                .call(rt)
                .map(IntoResponse::into_response as _)
                .right_future(),
        }
    }
}

impl<B, L, R, Lt, Rt> Handler<(Lt, Rt), B> for Or<L, R, Lt, Rt, B>
where
    L: HandlerCallWithExtractors<Lt, B> + Clone + Send + 'static,
    R: HandlerCallWithExtractors<Rt, B> + Clone + Send + 'static,
    Lt: FromRequest<B> + Send + 'static,
    Rt: FromRequest<B> + Send + 'static,
    Lt::Rejection: Send,
    Rt::Rejection: Send,
    B: Send + 'static,
{
    // this puts `futures_util` in our public API but thats fine in axum-extra
    type Future = BoxFuture<'static, Response>;

    fn call(self, req: Request<B>) -> Self::Future {
        Box::pin(async move {
            let mut req = RequestParts::new(req);

            if let Ok(lt) = req.extract::<Lt>().await {
                return self.lhs.call(lt).await;
            }

            if let Ok(rt) = req.extract::<Rt>().await {
                return self.rhs.call(rt).await;
            }

            StatusCode::NOT_FOUND.into_response()
        })
    }
}

impl<L, R, Lt, Rt, B> Copy for Or<L, R, Lt, Rt, B>
where
    L: Copy,
    R: Copy,
{
}

impl<L, R, Lt, Rt, B> Clone for Or<L, R, Lt, Rt, B>
where
    L: Clone,
    R: Clone,
{
    fn clone(&self) -> Self {
        Self {
            lhs: self.lhs.clone(),
            rhs: self.rhs.clone(),
            _marker: self._marker,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{
        extract::{Path, Query},
        routing::get,
        Router,
    };
    use serde::Deserialize;

    #[tokio::test]
    async fn works() {
        #[derive(Deserialize)]
        struct Params {
            a: String,
        }

        async fn one(Path(id): Path<u32>) -> String {
            id.to_string()
        }

        async fn two(Query(params): Query<Params>) -> String {
            params.a
        }

        async fn three() -> &'static str {
            "fallback"
        }

        let app = Router::new().route("/:id", get(one.or(two).or(three)));

        let client = TestClient::new(app);

        let res = client.get("/123").send().await;
        assert_eq!(res.text().await, "123");

        let res = client.get("/foo?a=bar").send().await;
        assert_eq!(res.text().await, "bar");

        let res = client.get("/foo").send().await;
        assert_eq!(res.text().await, "fallback");
    }
}
