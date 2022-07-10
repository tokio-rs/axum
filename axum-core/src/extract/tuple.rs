// this file is machine generated. Don't edit it!

use super::*;
use crate::response::Response;

#[async_trait]
impl<B, T1> FromRequest<Once, B> for (T1,)
where
    B: Send,
    T1: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1,))
    }
}
#[async_trait]
impl<B, T1, T2> FromRequest<Once, B> for (T1, T2)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2))
    }
}
#[async_trait]
impl<B, T1, T2, T3> FromRequest<Once, B> for (T1, T2, T3)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4> FromRequest<Once, B> for (T1, T2, T3, T4)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3, T4))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5> FromRequest<Once, B> for (T1, T2, T3, T4, T5)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3, T4, T5))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5, T6> FromRequest<Once, B> for (T1, T2, T3, T4, T5, T6)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T6 = T6::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3, T4, T5, T6))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5, T6, T7> FromRequest<Once, B> for (T1, T2, T3, T4, T5, T6, T7)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T6 = T6::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T7 = T7::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3, T4, T5, T6, T7))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5, T6, T7, T8> FromRequest<Once, B> for (T1, T2, T3, T4, T5, T6, T7, T8)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T6 = T6::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T7 = T7::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T8 = T8::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3, T4, T5, T6, T7, T8))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5, T6, T7, T8, T9> FromRequest<Once, B>
    for (T1, T2, T3, T4, T5, T6, T7, T8, T9)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T6 = T6::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T7 = T7::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T8 = T8::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T9 = T9::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3, T4, T5, T6, T7, T8, T9))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10> FromRequest<Once, B>
    for (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T6 = T6::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T7 = T7::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T8 = T8::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T9 = T9::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T10 = T10::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3, T4, T5, T6, T7, T8, T9, T10))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11> FromRequest<Once, B>
    for (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T6 = T6::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T7 = T7::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T8 = T8::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T9 = T9::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T10 = T10::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T11 = T11::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12> FromRequest<Once, B>
    for (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Mut, B> + Send,
    T12: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T6 = T6::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T7 = T7::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T8 = T8::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T9 = T9::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T10 = T10::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T11 = T11::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T12 = T12::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13> FromRequest<Once, B>
    for (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Mut, B> + Send,
    T12: FromRequest<Mut, B> + Send,
    T13: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T6 = T6::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T7 = T7::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T8 = T8::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T9 = T9::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T10 = T10::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T11 = T11::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T12 = T12::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T13 = T13::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14> FromRequest<Once, B>
    for (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14)
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Mut, B> + Send,
    T12: FromRequest<Mut, B> + Send,
    T13: FromRequest<Mut, B> + Send,
    T14: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T6 = T6::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T7 = T7::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T8 = T8::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T9 = T9::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T10 = T10::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T11 = T11::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T12 = T12::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T13 = T13::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T14 = T14::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15> FromRequest<Once, B>
    for (
        T1,
        T2,
        T3,
        T4,
        T5,
        T6,
        T7,
        T8,
        T9,
        T10,
        T11,
        T12,
        T13,
        T14,
        T15,
    )
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Mut, B> + Send,
    T12: FromRequest<Mut, B> + Send,
    T13: FromRequest<Mut, B> + Send,
    T14: FromRequest<Mut, B> + Send,
    T15: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T6 = T6::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T7 = T7::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T8 = T8::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T9 = T9::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T10 = T10::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T11 = T11::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T12 = T12::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T13 = T13::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T14 = T14::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T15 = T15::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((
            T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15,
        ))
    }
}
#[async_trait]
impl<B, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16> FromRequest<Once, B>
    for (
        T1,
        T2,
        T3,
        T4,
        T5,
        T6,
        T7,
        T8,
        T9,
        T10,
        T11,
        T12,
        T13,
        T14,
        T15,
        T16,
    )
where
    B: Send,
    T1: FromRequest<Mut, B> + Send,
    T2: FromRequest<Mut, B> + Send,
    T3: FromRequest<Mut, B> + Send,
    T4: FromRequest<Mut, B> + Send,
    T5: FromRequest<Mut, B> + Send,
    T6: FromRequest<Mut, B> + Send,
    T7: FromRequest<Mut, B> + Send,
    T8: FromRequest<Mut, B> + Send,
    T9: FromRequest<Mut, B> + Send,
    T10: FromRequest<Mut, B> + Send,
    T11: FromRequest<Mut, B> + Send,
    T12: FromRequest<Mut, B> + Send,
    T13: FromRequest<Mut, B> + Send,
    T14: FromRequest<Mut, B> + Send,
    T15: FromRequest<Mut, B> + Send,
    T16: FromRequest<Once, B> + Send,
{
    type Rejection = Response;
    #[allow(non_snake_case, unused_mut)]
    async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
        let mut req = req.to_mut();
        let T1 = T1::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T2 = T2::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T3 = T3::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T4 = T4::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T5 = T5::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T6 = T6::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T7 = T7::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T8 = T8::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T9 = T9::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T10 = T10::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T11 = T11::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T12 = T12::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T13 = T13::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T14 = T14::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let T15 = T15::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        let mut req = RequestParts::<Once, B>::new(req.into_request());
        let T16 = T16::from_request(&mut req)
            .await
            .map_err(|err| err.into_response())?;
        Ok((
            T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16,
        ))
    }
}
