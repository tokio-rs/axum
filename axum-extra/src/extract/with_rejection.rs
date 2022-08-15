use axum::async_trait;
use axum::extract::{FromRequest, RequestParts};
use axum::response::IntoResponse;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Copy, Default)]
pub struct WithRejection<E, R>(pub E, pub PhantomData<R>);

impl<E, R> WithRejection<E, R> {
    fn into_inner(self) -> E {
        self.0
    }
}

impl<E, R> Deref for WithRejection<E, R> {
    type Target = E;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<E, R> DerefMut for WithRejection<E, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl<B, E, R> FromRequest<B> for WithRejection<E, R>
where
    B: Send,
    E: FromRequest<B>,
    R: From<E::Rejection> + IntoResponse,
{
    type Rejection = R;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        match req.extract::<E>().await {
            Ok(extractor) => Ok(WithRejection(extractor, PhantomData)),
            Err(err) => Err(err.into()),
        }
    }
}
