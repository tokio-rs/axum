use super::HandlerCallWithExtractors;
use crate::either::Either;
use axum::{
    extract::{FromRequest, FromRequestParts},
    handler::Handler,
    http::Request,
    response::{IntoResponse, Response},
};
use futures_util::future::{BoxFuture, Either as EitherFuture, FutureExt, Map};
use std::{future::Future, marker::PhantomData};

/// [`Handler`] that runs one [`Handler`] and if that rejects it'll fallback to another
/// [`Handler`].
///
/// Created with [`HandlerCallWithExtractors::or`](super::HandlerCallWithExtractors::or).
#[allow(missing_debug_implementations)]
pub struct Or<L, R, Lt, Rt, S, B> {
    pub(super) lhs: L,
    pub(super) rhs: R,
    pub(super) _marker: PhantomData<fn() -> (Lt, Rt, S, B)>,
}

impl<S, B, L, R, Lt, Rt> HandlerCallWithExtractors<Either<Lt, Rt>, S, B> for Or<L, R, Lt, Rt, S, B>
where
    L: HandlerCallWithExtractors<Lt, S, B> + Send + 'static,
    R: HandlerCallWithExtractors<Rt, S, B> + Send + 'static,
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
        state: S,
    ) -> <Self as HandlerCallWithExtractors<Either<Lt, Rt>, S, B>>::Future {
        match extractors {
            Either::E1(lt) => self
                .lhs
                .call(lt, state)
                .map(IntoResponse::into_response as _)
                .left_future(),
            Either::E2(rt) => self
                .rhs
                .call(rt, state)
                .map(IntoResponse::into_response as _)
                .right_future(),
        }
    }
}

impl<S, B, L, R, Lt, Rt, M> Handler<(M, Lt, Rt), S, B> for Or<L, R, Lt, Rt, S, B>
where
    L: HandlerCallWithExtractors<Lt, S, B> + Clone + Send + 'static,
    R: HandlerCallWithExtractors<Rt, S, B> + Clone + Send + 'static,
    Lt: FromRequestParts<S> + Send + 'static,
    Rt: FromRequest<S, B, M> + Send + 'static,
    Lt::Rejection: Send,
    Rt::Rejection: Send,
    B: Send + 'static,
    S: Send + Sync + 'static,
{
    // this puts `futures_util` in our public API but thats fine in axum-extra
    type Future = BoxFuture<'static, Response>;

    fn call(self, req: Request<B>, state: S) -> Self::Future {
        Box::pin(async move {
            let (mut parts, body) = req.into_parts();

            if let Ok(lt) = Lt::from_request_parts(&mut parts, &state).await {
                return self.lhs.call(lt, state).await;
            }

            let req = Request::from_parts(parts, body);

            match Rt::from_request(req, &state).await {
                Ok(rt) => self.rhs.call(rt, state).await,
                Err(rejection) => rejection.into_response(),
            }
        })
    }
}

impl<L, R, Lt, Rt, S, B> Copy for Or<L, R, Lt, Rt, S, B>
where
    L: Copy,
    R: Copy,
{
}

impl<L, R, Lt, Rt, S, B> Clone for Or<L, R, Lt, Rt, S, B>
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
