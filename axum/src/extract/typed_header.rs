use super::{FromRequest, RequestParts};
use async_trait::async_trait;
use axum_core::response::{IntoResponse, IntoResponseParts, Response, ResponseParts};
use headers::HeaderMapExt;
use http::header::{HeaderName, HeaderValue};
use std::ops::Deref;

/// Extractor that extracts a typed header value from [`headers`].
///
/// In general, it's recommended to extract only the needed headers via `TypedHeader` rather than
/// removing all headers with the `HeaderMap` extractor.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{
///     extract::TypedHeader,
///     headers::UserAgent,
///     routing::get,
///     Router,
/// };
///
/// async fn users_teams_show(
///     TypedHeader(user_agent): TypedHeader<UserAgent>,
/// ) {
///     // ...
/// }
///
/// let app = Router::new().route("/users/:user_id/team/:team_id", get(users_teams_show));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "headers")))]
#[derive(Debug, Clone, Copy)]
pub struct TypedHeader<T>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for TypedHeader<T>
where
    T: headers::Header,
    B: Send,
{
    type Rejection = TypedHeaderRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        match req.headers().typed_try_get::<T>() {
            Ok(Some(value)) => Ok(Self(value)),
            Ok(None) => Err(TypedHeaderRejection {
                name: T::name(),
                reason: TypedHeaderRejectionReason::Missing,
            }),
            Err(err) => Err(TypedHeaderRejection {
                name: T::name(),
                reason: TypedHeaderRejectionReason::Error(err),
            }),
        }
    }
}

impl<T> Deref for TypedHeader<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> IntoResponseParts for TypedHeader<T>
where
    T: headers::Header,
{
    fn into_response_parts(self, res: &mut ResponseParts) {
        struct ExtendHeaders<'a> {
            res: &'a mut ResponseParts,
            key: &'static HeaderName,
        }

        impl<'a> Extend<HeaderValue> for ExtendHeaders<'a> {
            fn extend<T>(&mut self, iter: T)
            where
                T: IntoIterator<Item = HeaderValue>,
            {
                for value in iter {
                    self.res.append_header(self.key, value);
                }
            }
        }

        let mut extend = ExtendHeaders {
            res,
            key: T::name(),
        };

        self.0.encode(&mut extend);
    }
}

impl<T> IntoResponse for TypedHeader<T>
where
    T: headers::Header,
{
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        res.headers_mut().typed_insert(self.0);
        res
    }
}

/// Rejection used for [`TypedHeader`](super::TypedHeader).
#[cfg(feature = "headers")]
#[derive(Debug)]
pub struct TypedHeaderRejection {
    name: &'static http::header::HeaderName,
    reason: TypedHeaderRejectionReason,
}

impl TypedHeaderRejection {
    /// Name of the header that caused the rejection
    pub fn name(&self) -> &http::header::HeaderName {
        self.name
    }

    /// Reason why the header extraction has failed
    pub fn reason(&self) -> &TypedHeaderRejectionReason {
        &self.reason
    }
}

/// Additional information regarding a [`TypedHeaderRejection`](super::TypedHeaderRejection)
#[derive(Debug)]
#[non_exhaustive]
pub enum TypedHeaderRejectionReason {
    /// The header was missing from the HTTP request
    Missing,
    /// An error occured when parsing the header from the HTTP request
    Error(headers::Error),
}

impl IntoResponse for TypedHeaderRejection {
    fn into_response(self) -> Response {
        (http::StatusCode::BAD_REQUEST, self.to_string()).into_response()
    }
}

impl std::fmt::Display for TypedHeaderRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.reason {
            TypedHeaderRejectionReason::Missing => {
                write!(f, "Header of type `{}` was missing", self.name)
            }
            TypedHeaderRejectionReason::Error(err) => {
                write!(f, "{} ({})", err, self.name)
            }
        }
    }
}

impl std::error::Error for TypedHeaderRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.reason {
            TypedHeaderRejectionReason::Error(err) => Some(err),
            TypedHeaderRejectionReason::Missing => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{response::IntoResponse, routing::get, test_helpers::*, Router};

    #[tokio::test]
    async fn typed_header() {
        async fn handle(
            TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
        ) -> impl IntoResponse {
            user_agent.to_string()
        }

        let app = Router::new().route("/", get(handle));

        let client = TestClient::new(app);

        let res = client.get("/").header("user-agent", "foobar").send().await;
        let body = res.text().await;
        assert_eq!(body, "foobar");

        let res = client.get("/").send().await;
        let body = res.text().await;
        assert_eq!(body, "Header of type `user-agent` was missing");
    }
}
