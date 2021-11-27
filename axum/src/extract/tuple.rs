use super::{FromRequest, RequestParts};
use crate::{body::BoxBody, response::IntoResponse};
use async_trait::async_trait;
use http::Response;
use std::convert::Infallible;

#[async_trait]
impl<B> FromRequest<B> for ()
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(_: &mut RequestParts<B>) -> Result<(), Self::Rejection> {
        Ok(())
    }
}

macro_rules! impl_from_request {
    () => {};

    ( $($ty:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<B, $($ty,)*> FromRequest<B> for ($($ty,)*)
        where
            $( $ty: FromRequest<B> + Send, )*
            B: Send,
        {
            type Rejection = Response<BoxBody>;

            async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
                $( let $ty = $ty::from_request(req).await.map_err(|err| err.into_response())?; )*
                Ok(($($ty,)*))
            }
        }
    };
}

all_the_tuples!(impl_from_request);
