// TODO: Bring this back, and make sure it works with all the different possible permutations
// See https://github.com/tokio-rs/axum/pull/1277#issuecomment-1220358420 for details

// use super::{FromRequest, FromRequestParts};
// use crate::response::{IntoResponse, Response};
// use async_trait::async_trait;
// use http::request::{Parts, Request};
// use std::convert::Infallible;

// #[async_trait]
// impl<S> FromRequestParts<S> for ()
// where
//     S: Send + Sync,
// {
//     type Rejection = Infallible;

//     async fn from_request_parts(_: &mut Parts, _: &S) -> Result<(), Self::Rejection> {
//         Ok(())
//     }
// }

// macro_rules! impl_from_request {
//     (
//         [$($ty:ident),*], $last:ident
//     ) => {
//         #[async_trait]
//         #[allow(non_snake_case, unused_mut, unused_variables)]
//         impl<S, B, $($ty,)* $last> FromRequest<S, B> for ($($ty,)* $last,)
//         where
//             $( $ty: FromRequestParts<S> + Send, )*
//             $last: FromRequest<S, B> + Send,
//             B: Send + 'static,
//             S: Send + Sync,
//         {
//             type Rejection = Response;

//             async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
//                 let (mut parts, body) = req.into_parts();

//                 $(
//                     let $ty = $ty::from_request_parts(&mut parts, state).await.map_err(|err| err.into_response())?;
//                 )*

//                 let req = Request::from_parts(parts, body);

//                 let $last = $last::from_request(req, state).await.map_err(|err| err.into_response())?;

//                 Ok(($($ty,)* $last,))
//             }
//         }
//     };
// }

// impl_from_request!([], T1);
// impl_from_request!([T1], T2);
// impl_from_request!([T1, T2], T3);
// impl_from_request!([T1, T2, T3], T4);
// impl_from_request!([T1, T2, T3, T4], T5);
// impl_from_request!([T1, T2, T3, T4, T5], T6);
// impl_from_request!([T1, T2, T3, T4, T5, T6], T7);
// impl_from_request!([T1, T2, T3, T4, T5, T6, T7], T8);
// impl_from_request!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
// impl_from_request!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
// impl_from_request!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
// impl_from_request!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
// impl_from_request!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
// impl_from_request!(
//     [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13],
//     T14
// );
// impl_from_request!(
//     [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14],
//     T15
// );
// impl_from_request!(
//     [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15],
//     T16
// );
