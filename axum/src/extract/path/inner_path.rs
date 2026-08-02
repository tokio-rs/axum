use crate::extract::path::{get_params, serialize_path_params};
use crate::extract::rejection::InnerPathRejection;
use crate::extract::FromRequestParts;
use crate::routing::strip_prefix::count_captures;
use crate::util::PercentDecodedStr;
use axum_core::extract::{OptionalFromRequestParts, Request};
use axum_core::RequestPartsExt;
use http::request::Parts;
use serde_core::de::DeserializeOwned;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower_layer::{layer_fn, Layer};
use tower_service::Service;

/// Extractor that will get captures from the path of the innermost router
/// and parse them using [`serde`].
///
/// Any percent encoded parameters will be automatically decoded. The decoded
/// parameters must be valid UTF-8, otherwise `Path` will fail and return a `400
/// Bad Request` response.
///
/// # `InnerPath` vs [`Path`]
/// [`Path`] gives you every capture in the request path, including those matched by enclosing
/// [`nest`] calls. `InnerPath` gives you only the ones from the route the innermost router
/// matched. If the router isn't nested, or is nested only at static paths, `InnerPath` and [`Path`] see
/// exactly the same captures.
///
/// # `Option<InnerPath<T>>` behavior
///
/// You can use `InnerPath<Path<T>>` as an extractor to allow the same handler to
/// be used in a route with parameters that deserialize to `T`, and another
/// route with no parameters at all.
///
/// # Example
///
/// These examples assume the `serde` feature of the [`uuid`] crate is enabled.
///
/// One `InnerPath` can extract multiple captures. It is not necessary (and does
/// not work) to give a handler more than one `InnerPath` argument.
///
/// [`uuid`]: https://crates.io/crates/uuid
///
/// ```rust,no_run
/// use axum::{
///     extract::InnerPath,
///     routing::get,
///     Router,
/// };
/// use uuid::Uuid;
///
/// async fn user_info(InnerPath(user_id): InnerPath<Uuid>) {
///     // `user_id` is from the `{user_id}` capture,
///     // while `{tenant_id}` from the outer router isn't included,
///     // so this handler works no matter what it's nested under.
///     todo!()
/// }
///
/// let api = Router::new().route("/users/{user_id}", get(user_info));
/// let app = Router::new().nest("/tenants/{tenant_id}", api);
/// # let _: Router = app;
/// ```
///
/// Captured parameters also can be deserialized into any type that implements
/// [`serde::Deserialize`]. This includes tuples and structs:
///
/// ```rust,no_run
/// use axum::{
///     extract::InnerPath,
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
///     InnerPath(Params { user_id, team_id }): InnerPath<Params>,
/// ) {
///     // Capture parameters will be matched by struct field names
/// }
///
/// async fn users_teams_create(
///     InnerPath((user_id, team_id)): InnerPath<(String, String)>,
/// ) {
///     // When using tuples captures will be matched by their position in the route
/// }
///
/// let api = Router::new().route(
///     "/users/{user_id}/team/{team_id}",
///     get(users_teams_show).post(users_teams_create)
/// );
/// let app = Router::new().nest("/tenants/{tenant_id}", api);
/// # let _: Router = app;
/// ```
///
/// If you wish to capture all inner path parameters you can use `HashMap` or `Vec`:
///
/// ```rust,no_run
/// use axum::{
///     extract::InnerPath,
///     routing::get,
///     Router,
/// };
/// use std::collections::HashMap;
///
/// async fn params_map(
///     InnerPath(params): InnerPath<HashMap<String, String>>,
/// ) {
///     // `params` contains keys "team_id" and "user_id"
///     todo!()
/// }
///
/// async fn params_vec(
///     InnerPath(params): InnerPath<Vec<(String, String)>>,
/// ) {
///     todo!()
/// }
///
/// let api = Router::new()
///     .route("/users/{user_id}/team/{team_id}", get(params_map).post(params_vec));
/// let app = Router::new().nest("/tenants/{tenant_id}", api);
/// # let _: Router = app;
/// ```
///
/// # Providing detailed rejection output
///
/// If the URI cannot be deserialized into the target type the request will be rejected and an
/// error response will be returned. See [`customize-path-rejection`] for an example of how to customize that error.
///
/// [`serde`]: https://crates.io/crates/serde
/// [`serde::Deserialize`]: https://docs.rs/serde/1.0.127/serde/trait.Deserialize.html
/// [`customize-path-rejection`]: https://github.com/tokio-rs/axum/blob/main/examples/customize-path-rejection/src/main.rs
#[derive(Debug)]
pub struct InnerPath<T>(pub T);

axum_core::__impl_deref!(InnerPath);

impl<T, S> FromRequestParts<S> for InnerPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = InnerPathRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Non-T computation is in standalone function preventing monomorphization over T
        fn get_inner_params(
            parts: &Parts,
        ) -> Result<&[(Arc<str>, PercentDecodedStr)], InnerPathRejection> {
            let EnclosingCapturesCount(enclosing_captures_count) = parts
                .extensions
                .get::<EnclosingCapturesCount>()
                .copied()
                .unwrap_or_default();

            let inner_params = get_params::<InnerPathRejection>(parts)?
                .get(enclosing_captures_count..)
                .expect("Mismatch between url params and count of captures. This is bug in axum. Please file an issue.");

            Ok(inner_params)
        }

        get_inner_params(parts)
            .and_then(serialize_path_params)
            .map(Self)
    }
}

impl<T, S> OptionalFromRequestParts<S> for InnerPath<T>
where
    T: DeserializeOwned + Send + 'static,
    S: Send + Sync,
{
    type Rejection = InnerPathRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        match parts.extract::<Self>().await {
            Ok(Self(params)) => Ok(Some(Self(params))),
            Err(InnerPathRejection::FailedToDeserializePathParams(e))
                if e.kind().is_zero_params() =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug, Copy, Default, Clone)]
struct EnclosingCapturesCount(usize);

#[derive(Debug, Clone)]
pub(crate) struct CountEnclosingCaptures<S> {
    inner: S,
    additional_captures_count: usize,
}

impl<S> CountEnclosingCaptures<S> {
    pub(crate) fn layer(path_to_nest_at: &str) -> impl Layer<S, Service = Self> + Clone {
        let additional_captures_count = count_captures(path_to_nest_at);

        layer_fn(move |inner| Self {
            inner,
            additional_captures_count,
        })
    }
}

impl<S, B> Service<Request<B>> for CountEnclosingCaptures<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        let previous = req
            .extensions_mut()
            .get_or_insert_default::<EnclosingCapturesCount>();
        previous.0 += self.additional_captures_count;

        self.inner.call(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        extract::{Path, Request},
        middleware::{from_fn, Next},
        routing::get,
        test_helpers::*,
        Router,
    };
    use axum_core::response::Response;
    use http::StatusCode;
    use std::collections::HashMap;

    type Params = Vec<(String, String)>;

    fn params(pairs: &[(&str, &str)]) -> Params {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[crate::test]
    async fn without_nesting_matches_path() {
        let app = Router::new().route(
            "/users/{user_id}/team/{team_id}",
            get(
                |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                    let expected = params(&[("user_id", "42"), ("team_id", "1337")]);
                    assert_eq!(all, expected);
                    assert_eq!(inner, expected);
                    assert_eq!(inner, all);
                    "teams#show"
                },
            ),
        );
        let res = TestClient::new(app).get("/users/42/team/1337").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "teams#show");
    }

    #[crate::test]
    async fn static_nest_prefix_contributes_nothing() {
        let app = Router::new().nest(
            "/api",
            Router::new().route(
                "/users/{user_id}",
                get(
                    |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                        let expected = params(&[("user_id", "42")]);
                        assert_eq!(all, expected);
                        assert_eq!(inner, expected);
                        assert_eq!(inner, all);
                        "users#show"
                    },
                ),
            ),
        );
        let res = TestClient::new(app).get("/api/users/42").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "users#show");
    }

    #[crate::test]
    async fn dynamic_nest_prefix_is_excluded() {
        let app = Router::new().nest(
            "/org/{org}",
            Router::new().route(
                "/team/{team_id}",
                get(
                    |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                        assert_eq!(all, params(&[("org", "acme"), ("team_id", "1337")]));
                        assert_eq!(inner, params(&[("team_id", "1337")]));
                        "teams#show"
                    },
                ),
            ),
        );
        let res = TestClient::new(app).get("/org/acme/team/1337").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "teams#show");
    }

    #[crate::test]
    async fn nesting_mixing_static_and_dynamic_levels() {
        let app = Router::new().nest(
            "/api",
            Router::new().nest(
                "/{version}",
                Router::new().nest(
                    "/admin",
                    Router::new().route(
                        "/{id}",
                        get(
                            |Path(all): Path<Params>,
                             InnerPath(inner): InnerPath<Params>| async move {
                                assert_eq!(all, params(&[("version", "v2"), ("id", "42")]));
                                assert_eq!(inner, params(&[("id", "42")]));
                                "admin#show"
                            },
                        ),
                    ),
                ),
            ),
        );
        let res = TestClient::new(app).get("/api/v2/admin/42").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "admin#show");
    }

    #[crate::test]
    async fn offset_sums_captures_across_levels() {
        let app = Router::new().nest(
            "/{prefix_one}",
            Router::new().nest(
                "/{prefix_two}/{prefix_three}",
                Router::new().nest(
                    "/x/{prefix_four}",
                    Router::new().route(
                        "/{leaf_one}/{leaf_two}",
                        get(
                            |Path(all): Path<Params>,
                             InnerPath(inner): InnerPath<Params>| async move {
                                assert_eq!(
                                    all,
                                    params(&[
                                        ("prefix_one", "1"),
                                        ("prefix_two", "2"),
                                        ("prefix_three", "3"),
                                        ("prefix_four", "4"),
                                        ("leaf_one", "5"),
                                        ("leaf_two", "6"),
                                    ])
                                );
                                assert_eq!(
                                    inner,
                                    params(&[("leaf_one", "5"), ("leaf_two", "6")])
                                );
                                "leaf"
                            },
                        ),
                    ),
                ),
            ),
        );
        let res = TestClient::new(app).get("/1/2/3/x/4/5/6").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "leaf");
    }

    // `#[crate::test]` generates extra tests where `nest` calls are converted to `nest_service`.
    // This test mixes `nest` and `nest_service` at different levels.
    #[crate::test]
    async fn outer_nest_with_inner_nest_service() {
        let app = Router::new().nest(
            "/api",
            Router::new().nest_service(
                "/{version}",
                Router::new().route(
                    "/{id}",
                    get(
                        |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                            assert_eq!(all, params(&[("version", "v2"), ("id", "42")]));
                            assert_eq!(inner, params(&[("id", "42")]));
                            "items#show"
                        },
                    ),
                ),
            ),
        );
        let res = TestClient::new(app).get("/api/v2/42").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "items#show");
    }

    #[crate::test]
    async fn wildcard() {
        let app = Router::new().nest(
            "/org/{org}",
            Router::new().route(
                "/files/{*rest}",
                get(
                    |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                        assert_eq!(all, params(&[("org", "acme"), ("rest", "a/b/c")]));
                        assert_eq!(inner, params(&[("rest", "a/b/c")]));
                        "files#show"
                    },
                ),
            ),
        );
        let res = TestClient::new(app).get("/org/acme/files/a/b/c").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "files#show");
    }

    #[crate::test]
    async fn inner_route_without_captures() {
        let app = Router::new().nest(
            "/org/{org}",
            Router::new().route(
                "/static",
                get(
                    |Path(all): Path<Params>,
                     InnerPath(inner): InnerPath<Params>,
                     opt: Option<InnerPath<String>>| async move {
                        assert_eq!(all, params(&[("org", "acme")]));
                        assert_eq!(inner, params(&[]));
                        assert_eq!(opt.as_deref(), None);
                        "static"
                    },
                ),
            ),
        );
        let res = TestClient::new(app).get("/org/acme/static").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "static");
    }

    #[crate::test]
    async fn hash_map_only_contains_inner_captures() {
        let app = Router::new().nest(
            "/org/{org}",
            Router::new().route(
                "/team/{team_id}",
                get(
                    |Path(all): Path<HashMap<String, String>>,
                     InnerPath(inner): InnerPath<HashMap<String, String>>| async move {
                        assert_eq!(
                            all,
                            HashMap::from_iter(params(&[("org", "acme"), ("team_id", "1337")]))
                        );
                        assert_eq!(inner, HashMap::from_iter(params(&[("team_id", "1337")])));
                        "teams#show"
                    },
                ),
            ),
        );
        let res = TestClient::new(app).get("/org/acme/team/1337").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "teams#show");
    }

    #[crate::test]
    async fn same_router_nested_at_prefixes_of_different_arity() {
        fn resources() -> Router {
            Router::new().route(
                "/{id}/{version}",
                get(|InnerPath(inner): InnerPath<Params>| async move {
                    assert_eq!(inner, params(&[("id", "thing"), ("version", "3")]));
                    "resources#show"
                }),
            )
        }

        let app = Router::new()
            .nest("/global/{env}", resources())
            .nest("/customer/{customer_id}/{env}", resources());
        let client = TestClient::new(app);

        let res = client.get("/global/prod/thing/3").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "resources#show");

        let res = client.get("/customer/acme/prod/thing/3").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "resources#show");
    }

    #[crate::test]
    async fn in_nested_fallback() {
        let app = Router::new().nest(
            "/org/{org}",
            Router::new().fallback(get(
                |Path(all): Path<Params>,
                 InnerPath(inner): InnerPath<Params>,
                 opt: Option<InnerPath<String>>| async move {
                    assert_eq!(all, params(&[("org", "acme")]));
                    assert_eq!(inner, params(&[]));
                    assert_eq!(opt.as_deref(), None);
                    "fallback"
                },
            )),
        );
        let res = TestClient::new(app).get("/org/acme/does-not-exist").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "fallback");
    }

    #[crate::test]
    async fn in_nested_middleware() {
        async fn middleware(
            Path(all): Path<Params>,
            InnerPath(inner): InnerPath<Params>,
            req: Request,
            next: Next,
        ) -> Response {
            assert_eq!(all, params(&[("org", "acme"), ("team_id", "1337")]));
            assert_eq!(inner, params(&[("team_id", "1337")]));
            next.run(req).await
        }

        let app = Router::new().nest(
            "/org/{org}",
            Router::new()
                .route("/team/{team_id}", get(|| async { "teams#show" }))
                .layer(from_fn(middleware)),
        );
        let res = TestClient::new(app).get("/org/acme/team/1337").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "teams#show");
    }

    #[crate::test]
    async fn merge_preserves_per_route_offsets() {
        let unnested = Router::new().route(
            "/plain/{id}",
            get(
                |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                    let expected = params(&[("id", "0")]);
                    assert_eq!(all, expected);
                    assert_eq!(inner, expected);
                    "unnested"
                },
            ),
        );

        let nested_under_one_capture = Router::new().nest(
            "/org/{org}",
            Router::new().route(
                "/{id}",
                get(
                    |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                        assert_eq!(all, params(&[("org", "acme"), ("id", "1")]));
                        assert_eq!(inner, params(&[("id", "1")]));
                        "nested_under_one_capture"
                    },
                ),
            ),
        );

        let nested_under_two_captures = Router::new().nest(
            "/team/{team}/{env}",
            Router::new().route(
                "/{id}",
                get(
                    |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                        assert_eq!(
                            all,
                            params(&[("team", "blue"), ("env", "prod"), ("id", "2")])
                        );
                        assert_eq!(inner, params(&[("id", "2")]));
                        "nested_under_two_captures"
                    },
                ),
            ),
        );

        let client = TestClient::new(
            unnested
                .merge(nested_under_one_capture)
                .merge(nested_under_two_captures),
        );

        let res = client.get("/plain/0").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "unnested");

        let res = client.get("/org/acme/1").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "nested_under_one_capture");

        let res = client.get("/team/blue/prod/2").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "nested_under_two_captures");
    }

    #[crate::test]
    async fn nesting_a_merged_router_composes_offsets() {
        let already_nested = Router::new().nest(
            "/org/{org}",
            Router::new().route(
                "/{id}",
                get(
                    |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                        assert_eq!(all, params(&[("v", "2"), ("org", "acme"), ("id", "1")]));
                        assert_eq!(inner, params(&[("id", "1")]));
                        "already_nested"
                    },
                ),
            ),
        );

        let not_yet_nested = Router::new().route(
            "/plain/{id}",
            get(
                |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                    assert_eq!(all, params(&[("v", "2"), ("id", "0")]));
                    assert_eq!(inner, params(&[("id", "0")]));
                    "not_yet_nested"
                },
            ),
        );

        let app = Router::new().nest("/api/{v}", already_nested.merge(not_yet_nested));
        let client = TestClient::new(app);

        let res = client.get("/api/2/org/acme/1").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "already_nested");

        let res = client.get("/api/2/plain/0").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "not_yet_nested");
    }

    #[crate::test]
    async fn percent_decoding_still_applies() {
        let app = Router::new().nest(
            "/org/{org}",
            Router::new().route(
                "/{name}",
                get(
                    |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                        assert_eq!(all, params(&[("org", "acme"), ("name", "one two")]));
                        assert_eq!(inner, params(&[("name", "one two")]));
                        "names#show"
                    },
                ),
            ),
        );
        let res = TestClient::new(app).get("/org/acme/one%20two").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "names#show");
    }

    #[crate::test]
    async fn wrong_number_of_parameters_is_rejected() {
        let app = Router::new().nest(
            "/org/{org}",
            Router::new().route("/{id}", get(|_: InnerPath<(u32, u32)>| async move {})),
        );
        let res = TestClient::new(app).get("/org/acme/42").await;

        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            res.text().await,
            "Wrong number of path arguments for `Path`. Expected 2 but got 1"
        );
    }

    #[crate::test]
    async fn duplicate_capture_names_across_levels() {
        let app = Router::new().nest(
            "/org/{id}",
            Router::new().route(
                "/{id}",
                get(
                    |Path(all): Path<Params>, InnerPath(inner): InnerPath<Params>| async move {
                        assert_eq!(all, params(&[("id", "outer"), ("id", "inner")]));
                        assert_eq!(inner, params(&[("id", "inner")]));
                        "items#show"
                    },
                ),
            ),
        );
        let res = TestClient::new(app).get("/org/outer/inner").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "items#show");
    }

    #[crate::test]
    async fn invalid_utf8_in_nest_prefix_capture() {
        let app = Router::new().nest(
            "/org/{org}",
            Router::new().route(
                "/team/{team_id}",
                get(|_: InnerPath<Params>| async move { "teams#show" }),
            ),
        );
        let res = TestClient::new(app).get("/org/%FF/team/1337").await;

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert_eq!(res.text().await, "Invalid URL: Invalid UTF-8 in `org`");
    }

    #[crate::test]
    async fn invalid_utf8_in_inner_capture() {
        let app = Router::new().nest(
            "/org/{org}",
            Router::new().route(
                "/team/{team_id}",
                get(|_: InnerPath<Params>| async move { "teams#show" }),
            ),
        );
        let res = TestClient::new(app).get("/org/acme/team/%FF").await;

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert_eq!(res.text().await, "Invalid URL: Invalid UTF-8 in `team_id`");
    }
}
