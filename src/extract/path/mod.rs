mod de;

use super::{rejection::*, FromRequest};
use crate::{extract::RequestParts, routing::UrlParams};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::ops::{Deref, DerefMut};

/// Extractor that will get captures from the URL and parse them using [`serde`](https://crates.io/crates/serde).
///
/// # Example
///
/// ```rust,no_run
/// use axum::{extract::Path, prelude::*};
/// use uuid::Uuid;
///
/// async fn users_teams_show(
///     Path((user_id, team_id)): Path<(Uuid, Uuid)>,
/// ) {
///     // ...
/// }
///
/// let app = route("/users/:user_id/team/:team_id", get(users_teams_show));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If the path contains only one parameter, then you can omit the tuple.
///
/// ```rust,no_run
/// use axum::{extract::Path, prelude::*};
/// use uuid::Uuid;
///
/// async fn user_info(Path(user_id): Path<Uuid>) {
///     // ...
/// }
///
/// let app = route("/users/:user_id", get(user_info));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Path segments also can be deserialized into any type that implements [serde::Deserialize](https://docs.rs/serde/1.0.127/serde/trait.Deserialize.html).
/// Path segment labels will be matched with struct field names.
///
/// ```rust,no_run
/// use axum::{extract::Path, prelude::*};
/// use serde::Deserialize;
/// use uuid::Uuid;
///
/// #[derive(Deserialize)]
/// struct Params {
///     user_id: Uuid,
///     team_id: Uuid,
/// }
///
/// async fn users_teams_show(
///     Path(Params { user_id, team_id }): Path<Params>,
/// ) {
///     // ...
/// }
///
/// let app = route("/users/:user_id/team/:team_id", get(users_teams_show));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
#[derive(Debug)]
pub struct Path<T>(pub T);

impl<T> Deref for Path<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Path<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl<T, B> FromRequest<B> for Path<T>
where
    T: DeserializeOwned + Send,
    B: Send,
{
    type Rejection = PathParamsRejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        const EMPTY_URL_PARAMS: &UrlParams = &UrlParams(Vec::new());

        let url_params = if let Some(params) = req
            .extensions_mut()
            .and_then(|ext| ext.get::<Option<UrlParams>>())
        {
            params.as_ref().unwrap_or(EMPTY_URL_PARAMS)
        } else {
            return Err(MissingRouteParams.into());
        };

        T::deserialize(de::PathDeserializer::new(url_params))
            .map_err(|err| PathParamsRejection::InvalidPathParam(InvalidPathParam::new(err.0)))
            .map(Path)
    }
}
