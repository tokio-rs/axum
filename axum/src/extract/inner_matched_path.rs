use super::{
    matched_path::MatchedPath,
    nested_path::NestedPath,
    rejection::{MatchedPathMissing, MatchedPathRejection},
    FromRequestParts,
};
use axum_core::extract::OptionalFromRequestParts;
use http::request::Parts;
use std::{convert::Infallible, sync::Arc};

/// Access the path matched by the innermost router.
///
/// Unlike [`MatchedPath`], this excludes the path patterns contributed by
/// enclosing [`Router::nest`](crate::Router::nest) calls.
///
/// ```
/// use axum::{
///     Router,
///     extract::InnerMatchedPath,
///     routing::get,
/// };
///
/// let api = Router::new().route(
///     "/users/{id}",
///     get(|path: InnerMatchedPath| async move {
///         assert_eq!(path.as_str(), "/users/{id}");
///     }),
/// );
///
/// let app = Router::new().nest("/tenants/{tenant}", api);
/// # let _: Router = app;
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "matched-path")))]
#[derive(Clone, Debug)]
pub struct InnerMatchedPath(Arc<str>);

impl InnerMatchedPath {
    /// Returns a `str` representation of the path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_parts(parts: &Parts) -> Option<Self> {
        let matched_path = parts.extensions.get::<MatchedPath>()?;

        let inner = if let Some(nested_path) = parts.extensions.get::<NestedPath>() {
            let nested_path = nested_path.as_str().trim_end_matches('/');
            let inner = matched_path.as_str().strip_prefix(nested_path).unwrap_or_else(|| {
                panic!(
                    "nested path {nested_path:?} is not a prefix of matched path {:?}; this is a bug in axum",
                    matched_path.as_str()
                )
            });

            if inner.is_empty() { "/" } else { inner }
        } else {
            matched_path.as_str()
        };

        Some(Self(Arc::from(inner)))
    }
}

impl<S> FromRequestParts<S> for InnerMatchedPath
where
    S: Send + Sync,
{
    type Rejection = MatchedPathRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Self::from_parts(parts)
            .ok_or(MatchedPathRejection::MatchedPathMissing(MatchedPathMissing))
    }
}

impl<S> OptionalFromRequestParts<S> for InnerMatchedPath
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(Self::from_parts(parts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        extract::MatchedPath,
        routing::get,
        test_helpers::*,
        Router,
    };

    #[crate::test]
    async fn without_nesting_matches_matched_path() {
        let app = Router::new().route(
            "/users/{id}",
            get(|matched: MatchedPath, inner: InnerMatchedPath| async move {
                assert_eq!(matched.as_str(), "/users/{id}");
                assert_eq!(inner.as_str(), matched.as_str());
            }),
        );

        let res = TestClient::new(app).get("/users/42").await;
        assert_eq!(res.status(), http::StatusCode::OK);
    }

    #[crate::test]
    async fn excludes_enclosing_nest_path() {
        let api = Router::new().route(
            "/users/{id}",
            get(|matched: MatchedPath, inner: InnerMatchedPath| async move {
                assert_eq!(matched.as_str(), "/tenants/{tenant}/users/{id}");
                assert_eq!(inner.as_str(), "/users/{id}");
            }),
        );
        let app = Router::new().nest("/tenants/{tenant}", api);

        let res = TestClient::new(app).get("/tenants/acme/users/42").await;
        assert_eq!(res.status(), http::StatusCode::OK);
    }

    #[crate::test]
    async fn deeply_nested() {
        let api = Router::new().route(
            "/users/{id}",
            get(|inner: InnerMatchedPath| async move {
                assert_eq!(inner.as_str(), "/users/{id}");
            }),
        );
        let app = Router::new().nest(
            "/orgs/{org}",
            Router::new().nest("/teams/{team}", api),
        );

        let res = TestClient::new(app)
            .get("/orgs/acme/teams/platform/users/42")
            .await;
        assert_eq!(res.status(), http::StatusCode::OK);
    }

    #[crate::test]
    async fn inner_root_is_slash() {
        let api = Router::new().route(
            "/",
            get(|inner: InnerMatchedPath| async move {
                assert_eq!(inner.as_str(), "/");
            }),
        );
        let app = Router::new().nest("/tenants/{tenant}", api);

        let res = TestClient::new(app).get("/tenants/acme").await;
        assert_eq!(res.status(), http::StatusCode::OK);
    }
}
