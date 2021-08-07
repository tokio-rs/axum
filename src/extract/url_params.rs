use super::{rejection::*, FromRequest, RequestParts};
use async_trait::async_trait;
use std::{ops::Deref, str::FromStr};

/// Extractor that will get captures from the URL and parse them.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{extract::UrlParams, prelude::*};
/// use uuid::Uuid;
///
/// async fn users_teams_show(
///     UrlParams(params): UrlParams<(Uuid, Uuid)>,
/// ) {
///     let user_id: Uuid = params.0;
///     let team_id: Uuid = params.1;
///
///     // ...
/// }
///
/// let app = route("/users/:user_id/team/:team_id", get(users_teams_show));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Note that you can only have one URL params extractor per handler. If you
/// have multiple it'll response with `500 Internal Server Error`.
#[derive(Debug)]
#[deprecated(since = "0.1.3", note = "Use `axum::extract::Path` instead.")]
pub struct UrlParams<T>(pub T);

macro_rules! impl_parse_url {
    () => {};

    ( $head:ident, $($tail:ident),* $(,)? ) => {
        #[async_trait]
        #[allow(deprecated)]
        impl<B, $head, $($tail,)*> FromRequest<B> for UrlParams<($head, $($tail,)*)>
        where
            $head: FromStr + Send,
            $( $tail: FromStr + Send, )*
            B: Send,
        {
            type Rejection = UrlParamsRejection;

            #[allow(non_snake_case)]
            async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
                let params = if let Some(params) = req
                    .extensions_mut()
                    .and_then(|ext| {
                        ext.get_mut::<Option<crate::routing::UrlParams>>()
                    })
                {
                    if let Some(params) = params {
                        params.0.clone()
                    } else {
                        Default::default()
                    }
                } else {
                    return Err(MissingRouteParams.into())
                };

                if let [(_, $head), $((_, $tail),)*] = &*params {
                    let $head = if let Ok(x) = $head.as_str().parse::<$head>() {
                       x
                    } else {
                        return Err(InvalidUrlParam::new::<$head>().into());
                    };

                    $(
                        let $tail = if let Ok(x) = $tail.as_str().parse::<$tail>() {
                           x
                        } else {
                            return Err(InvalidUrlParam::new::<$tail>().into());
                        };
                    )*

                    Ok(UrlParams(($head, $($tail,)*)))
                } else {
                    Err(MissingRouteParams.into())
                }
            }
        }

        impl_parse_url!($($tail,)*);
    };
}

impl_parse_url!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

#[allow(deprecated)]
impl<T> Deref for UrlParams<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
