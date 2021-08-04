use super::{rejection::TypedHeaderRejection, FromRequest, RequestParts};
use async_trait::async_trait;
use headers::HeaderMap;
use std::ops::Deref;

/// Extractor that extracts a typed header value from [`headers`].
///
/// # Example
///
/// ```rust,no_run
/// use axum::{extract::TypedHeader, prelude::*};
/// use headers::UserAgent;
///
/// async fn users_teams_show(
///     TypedHeader(user_agent): TypedHeader<UserAgent>,
/// ) {
///     // ...
/// }
///
/// let app = route("/users/:user_id/team/:team_id", get(users_teams_show));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[cfg(feature = "headers")]
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
        let empty_headers = HeaderMap::new();
        let header_values = if let Some(headers) = req.headers() {
            headers.get_all(T::name())
        } else {
            empty_headers.get_all(T::name())
        };

        T::decode(&mut header_values.iter())
            .map(Self)
            .map_err(|err| TypedHeaderRejection {
                err,
                name: T::name(),
            })
    }
}

impl<T> Deref for TypedHeader<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
