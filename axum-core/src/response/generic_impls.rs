use super::{IntoResponse, IntoResponseParts, Response, ResponseParts};
use crate::body;
use http::StatusCode;

impl<T> IntoResponse for T
where
    T: IntoResponseParts,
{
    fn into_response(self) -> Response {
        let res = ().into_response();
        let mut parts = ResponseParts { res: Ok(res) };
        self.into_response_parts(&mut parts);

        match parts.res {
            Ok(res) => res,
            Err(err) => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(body::boxed(http_body::Full::from(err)))
                .unwrap(),
        }
    }
}

impl<R> IntoResponse for (StatusCode, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

macro_rules! impl_into_response {
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<R, $($ty,)*> IntoResponse for ($($ty),*, R)
        where
            $( $ty: IntoResponseParts, )*
            R: IntoResponse,
        {
            fn into_response(self) -> Response {
                let ($($ty),*, res) = self;

                let res = res.into_response();
                let mut parts = ResponseParts { res: Ok(res) };

                $(
                    $ty.into_response_parts(&mut parts);
                )*

                match parts.res {
                    Ok(res) => res,
                    Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
                }
            }
        }

        #[allow(non_snake_case)]
        impl<R, $($ty,)*> IntoResponse for (StatusCode, $($ty),*, R)
        where
            $( $ty: IntoResponseParts, )*
            R: IntoResponse,
        {
            fn into_response(self) -> Response {
                let (status, $($ty),*, res) = self;

                let res = res.into_response();
                let mut parts = ResponseParts { res: Ok(res) };

                $(
                    $ty.into_response_parts(&mut parts);
                )*

                match parts.res {
                    Ok(mut res) => {
                        *res.status_mut() = status;
                        res
                    }
                    Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
                }
            }
        }
    }
}

all_the_tuples!(impl_into_response);
