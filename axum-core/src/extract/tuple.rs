use super::{FromRequest, RequestParts};
use crate::response::{IntoResponse, Response};
use async_trait::async_trait;
use std::convert::Infallible;

#[async_trait]
impl<M, S, B> FromRequest<M, S, B> for ()
where
    B: Send,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(_: &mut RequestParts<M, S, B>) -> Result<(), Self::Rejection> {
        Ok(())
    }
}

// TODO(david): this will be fixed in a follow up PR
// macro_rules! impl_from_request {
//     () => {};

//     ( $($ty:ident),* $(,)? ) => {
//         #[async_trait]
//         #[allow(non_snake_case)]
//         impl<S, B, $($ty,)*> FromRequest<S, B> for ($($ty,)*)
//         where
//             $( $ty: FromRequest<S, B> + Send, )*
//             B: Send,
//             S: Send + Sync,
//         {
//             type Rejection = Response;

//             async fn from_request(req: &mut RequestParts<S, B>) -> Result<Self, Self::Rejection> {
//                 $( let $ty = $ty::from_request(req).await.map_err(|err| err.into_response())?; )*
//                 Ok(($($ty,)*))
//             }
//         }
//     };
// }

// all_the_tuples!(impl_from_request);
