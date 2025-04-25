//! `Either*` types for combining extractors or responses into a single type.
//!
//! # As an `FromRequestParts` extractor
//!
//! ```
//! use axum_extra::either::Either3;
//! use axum::{
//!     body::Bytes,
//!     Router,
//!     routing::get,
//!     extract::FromRequestParts,
//! };
//!
//! // extractors for checking permissions
//! struct AdminPermissions {}
//!
//! impl<S> FromRequestParts<S> for AdminPermissions
//! where
//!     S: Send + Sync,
//! {
//!     // check for admin permissions...
//!     # type Rejection = ();
//!     # async fn from_request_parts(parts: &mut axum::http::request::Parts, state: &S) -> Result<Self, Self::Rejection> {
//!     #     todo!()
//!     # }
//! }
//!
//! struct User {}
//!
//! impl<S> FromRequestParts<S> for User
//! where
//!     S: Send + Sync,
//! {
//!     // check for a logged in user...
//!     # type Rejection = ();
//!     # async fn from_request_parts(parts: &mut axum::http::request::Parts, state: &S) -> Result<Self, Self::Rejection> {
//!     #     todo!()
//!     # }
//! }
//!
//! async fn handler(
//!     body: Either3<AdminPermissions, User, ()>,
//! ) {
//!     match body {
//!         Either3::E1(admin) => { /* ... */ }
//!         Either3::E2(user) => { /* ... */ }
//!         Either3::E3(guest) => { /* ... */ }
//!     }
//! }
//! #
//! # let _: axum::routing::MethodRouter = axum::routing::get(handler);
//! ```
//!
//! Note that if all the inner extractors reject the request, the rejection from the last
//! extractor will be returned. For the example above that would be [`BytesRejection`].
//!
//! # As an `FromRequest` extractor
//!
//! In the following example, we can first try to deserialize the payload as JSON, if that fails try
//! to interpret it as a UTF-8 string, and lastly just take the raw bytes.
//!
//! It might be preferable to instead extract `Bytes` directly and then fallibly convert them to
//! `String` and then deserialize the data inside the handler.
//!
//! ```
//! use axum_extra::either::Either3;
//! use axum::{
//!     body::Bytes,
//!     Json,
//!     Router,
//!     routing::get,
//!     extract::FromRequestParts,
//! };
//!
//! #[derive(serde::Deserialize)]
//! struct Payload {
//!     user: String,
//!     request_id: u32,
//! }
//!
//! async fn handler(
//!     body: Either3<Json<Payload>, String, Bytes>,
//! ) {
//!     match body {
//!         Either3::E1(json) => { /* ... */ }
//!         Either3::E2(string) => { /* ... */ }
//!         Either3::E3(bytes) => { /* ... */ }
//!     }
//! }
//! #
//! # let _: axum::routing::MethodRouter = axum::routing::get(handler);
//! ```
//! # As a response
//!
//! ```
//! use axum_extra::either::Either3;
//! use axum::{Json, http::StatusCode, response::IntoResponse};
//! use serde_json::{Value, json};
//!
//! async fn handler() -> Either3<Json<Value>, &'static str, StatusCode> {
//!     if something() {
//!         Either3::E1(Json(json!({ "data": "..." })))
//!     } else if something_else() {
//!         Either3::E2("foobar")
//!     } else {
//!         Either3::E3(StatusCode::NOT_FOUND)
//!     }
//! }
//!
//! fn something() -> bool {
//!     // ...
//!     # false
//! }
//!
//! fn something_else() -> bool {
//!     // ...
//!     # false
//! }
//! #
//! # let _: axum::routing::MethodRouter = axum::routing::get(handler);
//! ```
//!
//! The general recommendation is to use [`IntoResponse::into_response`] to return different response
//! types, but if you need to preserve the exact type then `Either*` works as well.
//!
//! [`BytesRejection`]: axum::extract::rejection::BytesRejection
//! [`IntoResponse::into_response`]: https://docs.rs/axum/0.8/axum/response/index.html#returning-different-response-types

use std::task::{Context, Poll};

use axum::{
    extract::{rejection::BytesRejection, FromRequest, FromRequestParts, Request},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use http::request::Parts;
use tower_layer::Layer;
use tower_service::Service;

/// Combines two extractors or responses into a single type.
///
/// See the [module docs](self) for examples.
#[derive(Debug, Clone)]
#[must_use]
pub enum Either<E1, E2> {
    #[allow(missing_docs)]
    E1(E1),
    #[allow(missing_docs)]
    E2(E2),
}

/// Combines three extractors or responses into a single type.
///
/// See the [module docs](self) for examples.
#[derive(Debug, Clone)]
#[must_use]
pub enum Either3<E1, E2, E3> {
    #[allow(missing_docs)]
    E1(E1),
    #[allow(missing_docs)]
    E2(E2),
    #[allow(missing_docs)]
    E3(E3),
}

/// Combines four extractors or responses into a single type.
///
/// See the [module docs](self) for examples.
#[derive(Debug, Clone)]
#[must_use]
pub enum Either4<E1, E2, E3, E4> {
    #[allow(missing_docs)]
    E1(E1),
    #[allow(missing_docs)]
    E2(E2),
    #[allow(missing_docs)]
    E3(E3),
    #[allow(missing_docs)]
    E4(E4),
}

/// Combines five extractors or responses into a single type.
///
/// See the [module docs](self) for examples.
#[derive(Debug, Clone)]
#[must_use]
pub enum Either5<E1, E2, E3, E4, E5> {
    #[allow(missing_docs)]
    E1(E1),
    #[allow(missing_docs)]
    E2(E2),
    #[allow(missing_docs)]
    E3(E3),
    #[allow(missing_docs)]
    E4(E4),
    #[allow(missing_docs)]
    E5(E5),
}

/// Combines six extractors or responses into a single type.
///
/// See the [module docs](self) for examples.
#[derive(Debug, Clone)]
#[must_use]
pub enum Either6<E1, E2, E3, E4, E5, E6> {
    #[allow(missing_docs)]
    E1(E1),
    #[allow(missing_docs)]
    E2(E2),
    #[allow(missing_docs)]
    E3(E3),
    #[allow(missing_docs)]
    E4(E4),
    #[allow(missing_docs)]
    E5(E5),
    #[allow(missing_docs)]
    E6(E6),
}

/// Combines seven extractors or responses into a single type.
///
/// See the [module docs](self) for examples.
#[derive(Debug, Clone)]
#[must_use]
pub enum Either7<E1, E2, E3, E4, E5, E6, E7> {
    #[allow(missing_docs)]
    E1(E1),
    #[allow(missing_docs)]
    E2(E2),
    #[allow(missing_docs)]
    E3(E3),
    #[allow(missing_docs)]
    E4(E4),
    #[allow(missing_docs)]
    E5(E5),
    #[allow(missing_docs)]
    E6(E6),
    #[allow(missing_docs)]
    E7(E7),
}

/// Combines eight extractors or responses into a single type.
///
/// See the [module docs](self) for examples.
#[derive(Debug, Clone)]
#[must_use]
pub enum Either8<E1, E2, E3, E4, E5, E6, E7, E8> {
    #[allow(missing_docs)]
    E1(E1),
    #[allow(missing_docs)]
    E2(E2),
    #[allow(missing_docs)]
    E3(E3),
    #[allow(missing_docs)]
    E4(E4),
    #[allow(missing_docs)]
    E5(E5),
    #[allow(missing_docs)]
    E6(E6),
    #[allow(missing_docs)]
    E7(E7),
    #[allow(missing_docs)]
    E8(E8),
}

/// Rejection used for [`Either`], [`Either3`], etc.
///
/// Contains one variant for a case when the whole request could not be loaded and one variant
/// containing the rejection of the last variant if all extractors failed..
#[derive(Debug)]
pub enum EitherRejection<E> {
    /// Buffering of the request body failed.
    Bytes(BytesRejection),

    /// All extractors failed. This contains the error returned by the last extractor.
    LastRejection(E),
}

impl<E: IntoResponse> IntoResponse for EitherRejection<E> {
    fn into_response(self) -> Response {
        match self {
            EitherRejection::Bytes(rejection) => rejection.into_response(),
            EitherRejection::LastRejection(rejection) => rejection.into_response(),
        }
    }
}

macro_rules! impl_traits_for_either {
    (
        $either:ident =>
        [$($ident:ident),* $(,)?],
        $last:ident $(,)?
    ) => {
        impl<S, $($ident),*, $last> FromRequestParts<S> for $either<$($ident),*, $last>
        where
            $($ident: FromRequestParts<S>),*,
            $last: FromRequestParts<S>,
            S: Send + Sync,
        {
            type Rejection = $last::Rejection;

            async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
                $(
                    if let Ok(value) = <$ident as FromRequestParts<S>>::from_request_parts(parts, state).await {
                        return Ok(Self::$ident(value));
                    }
                )*

                <$last as FromRequestParts<S>>::from_request_parts(parts, state).await.map(Self::$last)
            }
        }

        impl<S, $($ident),*, $last> FromRequest<S> for $either<$($ident),*, $last>
        where
            S: Send + Sync,
            $($ident: FromRequest<S>),*,
            $last: FromRequest<S>,
            $($ident::Rejection: Send),*,
            $last::Rejection: IntoResponse + Send,
        {
            type Rejection = EitherRejection<$last::Rejection>;

            async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
                let (parts, body) = req.into_parts();
                let bytes = Bytes::from_request(Request::from_parts(parts.clone(), body), state)
                    .await
                    .map_err(EitherRejection::Bytes)?;

                $(
                    let req = Request::from_parts(
                        parts.clone(),
                        axum::body::Body::new(http_body_util::Full::new(bytes.clone())),
                    );
                    if let Ok(extracted) = $ident::from_request(req, state).await {
                        return Ok(Self::$ident(extracted));
                    }
                )*

                let req = Request::from_parts(
                    parts.clone(),
                    axum::body::Body::new(http_body_util::Full::new(bytes.clone())),
                );
                match $last::from_request(req, state).await {
                    Ok(extracted) => Ok(Self::$last(extracted)),
                    Err(error) => Err(EitherRejection::LastRejection(error)),
                }
            }
        }

        impl<$($ident),*, $last> IntoResponse for $either<$($ident),*, $last>
        where
            $($ident: IntoResponse),*,
            $last: IntoResponse,
        {
            fn into_response(self) -> Response {
                match self {
                    $( Self::$ident(value) => value.into_response(), )*
                    Self::$last(value) => value.into_response(),
                }
            }
        }
    };
}

impl_traits_for_either!(Either => [E1], E2);
impl_traits_for_either!(Either3 => [E1, E2], E3);
impl_traits_for_either!(Either4 => [E1, E2, E3], E4);
impl_traits_for_either!(Either5 => [E1, E2, E3, E4], E5);
impl_traits_for_either!(Either6 => [E1, E2, E3, E4, E5], E6);
impl_traits_for_either!(Either7 => [E1, E2, E3, E4, E5, E6], E7);
impl_traits_for_either!(Either8 => [E1, E2, E3, E4, E5, E6, E7], E8);

impl<E1, E2, S> Layer<S> for Either<E1, E2>
where
    E1: Layer<S>,
    E2: Layer<S>,
{
    type Service = Either<E1::Service, E2::Service>;

    fn layer(&self, inner: S) -> Self::Service {
        match self {
            Either::E1(layer) => Either::E1(layer.layer(inner)),
            Either::E2(layer) => Either::E2(layer.layer(inner)),
        }
    }
}

impl<R, E1, E2> Service<R> for Either<E1, E2>
where
    E1: Service<R>,
    E2: Service<R, Response = E1::Response, Error = E1::Error>,
{
    type Response = E1::Response;
    type Error = E1::Error;
    type Future = futures_util::future::Either<E1::Future, E2::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self {
            Either::E1(inner) => inner.poll_ready(cx),
            Either::E2(inner) => inner.poll_ready(cx),
        }
    }

    fn call(&mut self, req: R) -> Self::Future {
        match self {
            Either::E1(inner) => futures_util::future::Either::Left(inner.call(req)),
            Either::E2(inner) => futures_util::future::Either::Right(inner.call(req)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;

    use axum::body::Body;
    use axum::extract::rejection::StringRejection;
    use axum::extract::{FromRequest, Request, State};
    use bytes::Bytes;
    use http_body_util::Full;

    use super::*;

    struct False;

    impl<S> FromRequestParts<S> for False {
        type Rejection = ();

        fn from_request_parts(
            _parts: &mut Parts,
            _state: &S,
        ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
            std::future::ready(Err(()))
        }
    }

    #[tokio::test]
    async fn either_from_request() {
        // The body is by design not valid UTF-8.
        let request = Request::new(Body::new(Full::new(Bytes::from_static(&[255]))));

        let either = Either4::<String, String, Request, Bytes>::from_request(request, &())
            .await
            .unwrap();

        assert!(matches!(either, Either4::E3(_)));
    }

    #[tokio::test]
    async fn either_from_request_rejection() {
        // The body is by design not valid UTF-8.
        let request = Request::new(Body::new(Full::new(Bytes::from_static(&[255]))));

        let either = Either::<String, String>::from_request(request, &())
            .await
            .unwrap_err();

        assert!(matches!(
            either,
            EitherRejection::LastRejection(StringRejection::InvalidUtf8(_))
        ));
    }

    #[tokio::test]
    async fn either_from_request_parts() {
        let (mut parts, _) = Request::new(Body::empty()).into_parts();

        let either = Either3::<False, False, State<()>>::from_request_parts(&mut parts, &())
            .await
            .unwrap();

        assert!(matches!(either, Either3::E3(State(()))));
    }
}
