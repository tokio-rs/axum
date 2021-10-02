use super::{FromRequest, RequestParts};
use crate::{
    body::{box_body, BoxBody},
    response::IntoResponse,
};
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
                $( let $ty = $ty::from_request(req).await.map_err(|err| err.into_response().map(box_body))?; )*
                Ok(($($ty,)*))
            }
        }
    };
}

impl_from_request!(T1);
impl_from_request!(T1, T2);
impl_from_request!(T1, T2, T3);
impl_from_request!(T1, T2, T3, T4);
impl_from_request!(T1, T2, T3, T4, T5);
impl_from_request!(T1, T2, T3, T4, T5, T6);
impl_from_request!(T1, T2, T3, T4, T5, T6, T7);
impl_from_request!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_from_request!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_from_request!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_from_request!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_from_request!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_from_request!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_from_request!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_from_request!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
impl_from_request!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);
