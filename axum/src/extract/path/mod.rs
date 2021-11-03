mod de;

use super::{rejection::*, FromRequest};
use crate::{
    extract::RequestParts,
    routing::{InvalidUtf8InPathParam, UrlParams},
};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

/// Extractor that will get captures from the URL and parse them using
/// [`serde`].
///
/// Any percent encoded parameters will be automatically decoded. The decoded
/// parameters must be valid UTF-8, otherwise `Path` will fail and return a `400
/// Bad Request` response.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{
///     extract::Path,
///     routing::get,
///     Router,
/// };
/// use uuid::Uuid;
///
/// async fn users_teams_show(
///     Path((user_id, team_id)): Path<(Uuid, Uuid)>,
/// ) {
///     // ...
/// }
///
/// let app = Router::new().route("/users/:user_id/team/:team_id", get(users_teams_show));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If the path contains only one parameter, then you can omit the tuple.
///
/// ```rust,no_run
/// use axum::{
///     extract::Path,
///     routing::get,
///     Router,
/// };
/// use uuid::Uuid;
///
/// async fn user_info(Path(user_id): Path<Uuid>) {
///     // ...
/// }
///
/// let app = Router::new().route("/users/:user_id", get(user_info));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// Path segments also can be deserialized into any type that implements
/// [`serde::Deserialize`]. Path segment labels will be matched with struct
/// field names.
///
/// ```rust,no_run
/// use axum::{
///     extract::Path,
///     routing::get,
///     Router,
/// };
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
/// let app = Router::new().route("/users/:user_id/team/:team_id", get(users_teams_show));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// If you wish to capture all path parameters you can use `HashMap` or `Vec`:
///
/// ```rust,no_run
/// use axum::{
///     extract::Path,
///     routing::get,
///     Router,
/// };
/// use std::collections::HashMap;
///
/// async fn params_map(
///     Path(params): Path<HashMap<String, String>>,
/// ) {
///     // ...
/// }
///
/// async fn params_vec(
///     Path(params): Path<Vec<(String, String)>>,
/// ) {
///     // ...
/// }
///
/// let app = Router::new()
///     .route("/users/:user_id/team/:team_id", get(params_map).post(params_vec));
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// [`serde`]: https://crates.io/crates/serde
/// [`serde::Deserialize`]: https://docs.rs/serde/1.0.127/serde/trait.Deserialize.html
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
        let params = match req
            .extensions_mut()
            .and_then(|ext| ext.get::<Option<UrlParams>>())
        {
            Some(Some(UrlParams(Ok(params)))) => Cow::Borrowed(params),
            Some(Some(UrlParams(Err(InvalidUtf8InPathParam { key })))) => {
                return Err(InvalidPathParam::new(key.as_str()).into())
            }
            Some(None) => Cow::Owned(Vec::new()),
            None => {
                return Err(MissingRouteParams.into());
            }
        };

        T::deserialize(de::PathDeserializer::new(&*params))
            .map_err(|err| PathParamsRejection::InvalidPathParam(InvalidPathParam::new(err.0)))
            .map(Path)
    }
}

#[cfg(test)]
mod tests {
    use http::StatusCode;

    use super::*;
    use crate::test_helpers::*;
    use crate::{routing::get, Router};
    use std::collections::HashMap;

    #[tokio::test]
    async fn extracting_url_params() {
        let app = Router::new().route(
            "/users/:id",
            get(|Path(id): Path<i32>| async move {
                assert_eq!(id, 42);
            })
            .post(|Path(params_map): Path<HashMap<String, i32>>| async move {
                assert_eq!(params_map.get("id").unwrap(), &1337);
            }),
        );

        let client = TestClient::new(app);

        let res = client.get("/users/42").send().await;
        assert_eq!(res.status(), StatusCode::OK);

        let res = client.post("/users/1337").send().await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn extracting_url_params_multiple_times() {
        let app = Router::new().route("/users/:id", get(|_: Path<i32>, _: Path<String>| async {}));

        let client = TestClient::new(app);

        let res = client.get("/users/42").send().await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn percent_decoding() {
        let app = Router::new().route(
            "/:key",
            get(|Path(param): Path<String>| async move { param }),
        );

        let client = TestClient::new(app);

        let res = client.get("/one%20two").send().await;

        assert_eq!(res.text().await, "one two");
    }

    #[tokio::test]
    async fn supports_128_bit_numbers() {
        let app = Router::new()
            .route(
                "/i/:key",
                get(|Path(param): Path<i128>| async move { param.to_string() }),
            )
            .route(
                "/u/:key",
                get(|Path(param): Path<u128>| async move { param.to_string() }),
            );

        let client = TestClient::new(app);

        let res = client.get("/i/123").send().await;
        assert_eq!(res.text().await, "123");

        let res = client.get("/u/123").send().await;
        assert_eq!(res.text().await, "123");
    }

    #[tokio::test]
    async fn wildcard() {
        let app = Router::new()
            .route(
                "/foo/*rest",
                get(|Path(param): Path<String>| async move { param }),
            )
            .route(
                "/bar/*rest",
                get(|Path(params): Path<HashMap<String, String>>| async move {
                    params.get("rest").unwrap().clone()
                }),
            );

        let client = TestClient::new(app);

        let res = client.get("/foo/bar/baz").send().await;
        assert_eq!(res.text().await, "/bar/baz");

        let res = client.get("/bar/baz/qux").send().await;
        assert_eq!(res.text().await, "/baz/qux");
    }

    #[tokio::test]
    async fn captures_dont_match_empty_segments() {
        let app = Router::new().route("/:key", get(|| async {}));

        let client = TestClient::new(app);

        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        let res = client.get("/foo").send().await;
        assert_eq!(res.status(), StatusCode::OK);
    }
}
