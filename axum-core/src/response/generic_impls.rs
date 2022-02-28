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

// TODO(david): macroify these impls

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

impl<R, T1> IntoResponse for (T1, R)
where
    T1: IntoResponseParts,
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let res = self.1.into_response();
        let mut parts = ResponseParts { res: Ok(res) };

        self.0.into_response_parts(&mut parts);

        match parts.res {
            Ok(res) => res,
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
        }
    }
}

impl<R, T1> IntoResponse for (StatusCode, T1, R)
where
    T1: IntoResponseParts,
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let res = self.2.into_response();
        let mut parts = ResponseParts { res: Ok(res) };

        self.1.into_response_parts(&mut parts);

        match parts.res {
            Ok(mut res) => {
                *res.status_mut() = self.0;
                res
            }
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
        }
    }
}

impl<R, T1, T2> IntoResponse for (T1, T2, R)
where
    T1: IntoResponseParts,
    T2: IntoResponseParts,
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let res = self.2.into_response();
        let mut parts = ResponseParts { res: Ok(res) };

        self.0.into_response_parts(&mut parts);
        self.1.into_response_parts(&mut parts);

        match parts.res {
            Ok(res) => res,
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
        }
    }
}

impl<R, T1, T2> IntoResponse for (StatusCode, T1, T2, R)
where
    T1: IntoResponseParts,
    T2: IntoResponseParts,
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let res = self.3.into_response();
        let mut parts = ResponseParts { res: Ok(res) };

        self.1.into_response_parts(&mut parts);
        self.2.into_response_parts(&mut parts);

        match parts.res {
            Ok(mut res) => {
                *res.status_mut() = self.0;
                res
            }
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
        }
    }
}

impl<R, T1, T2, T3> IntoResponse for (T1, T2, T3, R)
where
    T1: IntoResponseParts,
    T2: IntoResponseParts,
    T3: IntoResponseParts,
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let res = self.3.into_response();
        let mut parts = ResponseParts { res: Ok(res) };

        self.0.into_response_parts(&mut parts);
        self.1.into_response_parts(&mut parts);
        self.2.into_response_parts(&mut parts);

        match parts.res {
            Ok(res) => res,
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
        }
    }
}

impl<R, T1, T2, T3> IntoResponse for (StatusCode, T1, T2, T3, R)
where
    T1: IntoResponseParts,
    T2: IntoResponseParts,
    T3: IntoResponseParts,
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let res = self.4.into_response();
        let mut parts = ResponseParts { res: Ok(res) };

        self.1.into_response_parts(&mut parts);
        self.2.into_response_parts(&mut parts);
        self.3.into_response_parts(&mut parts);

        match parts.res {
            Ok(mut res) => {
                *res.status_mut() = self.0;
                res
            }
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
        }
    }
}
