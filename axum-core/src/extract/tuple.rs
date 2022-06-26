use super::{FromRequest, Mut, Once, RequestParts};
use crate::response::{IntoResponse, Response};
use async_trait::async_trait;
use std::convert::Infallible;

#[async_trait]
impl<R, B> FromRequest<R, B> for ()
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(_: &mut RequestParts<R, B>) -> Result<Self, Self::Rejection> {
        Ok(())
    }
}

// TODO(david): macroify this
#[async_trait]
impl<B, T1> FromRequest<Once, B> for (T1,)
where
    T1: FromRequest<Once, B> + Send,
    B: Send,
{
    type Rejection = Response;

    #[allow(non_snake_case)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let T1: T1 = T1::from_request(req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1,))
    }
}

#[async_trait]
impl<B, T1, T2> FromRequest<Once, B> for (T1, T2)
where
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Once, B> + Send,
    B: Send,
{
    type Rejection = Response;

    #[allow(non_snake_case)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();

        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;

        let mut req = RequestParts::new(req.into_request());

        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;

        Ok((T1, T2))
    }
}

#[async_trait]
impl<B, T1, T2, T3> FromRequest<Once, B> for (T1, T2, T3)
where
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Once, B> + Send,
    B: Send,
{
    type Rejection = Response;

    #[allow(non_snake_case)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();

        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;

        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;

        let mut req = RequestParts::new(req.into_request());

        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;

        Ok((T1, T2, T3))
    }
}

// macro_rules! impl_from_request {
//     () => {};

//     ( $($ty:ident),* $(,)? ) => {
//         #[async_trait]
//         #[allow(non_snake_case)]
//         impl<B, $($ty,)*> FromRequest<Mut, B> for ($($ty,)*)
//         where
//             $( $ty: FromRequest<B> + Send, )*
//             B: Send,
//         {
//             type Rejection = Response;

//             async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
//                 $( let $ty = $ty::from_request(req).await.map_err(|err| err.into_response())?; )*
//                 Ok(($($ty,)*))
//             }
//         }
//     };
// }

// all_the_tuples!(impl_from_request);
