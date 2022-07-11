use super::{FromRequest, RequestParts};
use crate::response::{IntoResponse, Response};
use async_trait::async_trait;
use std::convert::Infallible;

#[async_trait]
impl FromRequest for () {
    type Rejection = Infallible;

    async fn from_request(_: &mut RequestParts) -> Result<(), Self::Rejection> {
        Ok(())
    }
}

macro_rules! impl_from_request {
    ( $($ty:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(non_snake_case)]
        impl<$($ty,)*> FromRequest for ($($ty,)*)
        where
            $( $ty: FromRequest + Send, )*
        {
            type Rejection = Response;

            async fn from_request(req: &mut RequestParts) -> Result<Self, Self::Rejection> {
                $( let $ty = $ty::from_request(req).await.map_err(|err| err.into_response())?; )*
                Ok(($($ty,)*))
            }
        }
    };
}

all_the_tuples!(impl_from_request);
