use super::*;
use crate::middleware::{map_request, map_response};

#[crate::test]
async fn basic() {
    let app = Router::new()
        .route("/foo", get(|| async {}))
        .fallback(|| async { "fallback" });

    let client = TestClient::new(app);

    assert_eq!(client.get("/foo").await.status(), StatusCode::OK);

    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "fallback");
}

#[crate::test]
async fn nest() {
    let app = Router::new()
        .nest("/foo", Router::new().route("/bar", get(|| async {})))
        .fallback(|| async { "fallback" });

    let client = TestClient::new(app);

    assert_eq!(client.get("/foo/bar").await.status(), StatusCode::OK);

    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "fallback");
}

#[crate::test]
async fn two() {
    let app = Router::new()
        .route("/first", get(|| async {}))
        .route("/second", get(|| async {}))
        .fallback(get(|| async { "fallback" }));
    let client = TestClient::new(app);
    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "fallback");
}

#[crate::test]
async fn or() {
    let one = Router::new().route("/one", get(|| async {}));
    let two = Router::new().route("/two", get(|| async {}));

    let app = one.merge(two).fallback(|| async { "fallback" });

    let client = TestClient::new(app);

    assert_eq!(client.get("/one").await.status(), StatusCode::OK);
    assert_eq!(client.get("/two").await.status(), StatusCode::OK);

    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "fallback");
}

#[crate::test]
async fn fallback_accessing_state() {
    let app = Router::new()
        .fallback(|State(state): State<&'static str>| async move { state })
        .with_state("state");

    let client = TestClient::new(app);

    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "state");
}

async fn inner_fallback() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "inner")
}

async fn outer_fallback() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "outer")
}

#[crate::test]
async fn nested_router_inherits_fallback() {
    let inner = Router::new();
    let app = Router::new().nest("/foo", inner).fallback(outer_fallback);

    let client = TestClient::new(app);

    let res = client.get("/foo/bar").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn doesnt_inherit_fallback_if_overridden() {
    let inner = Router::new().fallback(inner_fallback);
    let app = Router::new().nest("/foo", inner).fallback(outer_fallback);

    let client = TestClient::new(app);

    let res = client.get("/foo/bar").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "inner");

    let res = client.get("/").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn deeply_nested_inherit_from_top() {
    let app = Router::new()
        .nest("/foo", Router::new().nest("/bar", Router::new()))
        .fallback(outer_fallback);

    let client = TestClient::new(app);

    let res = client.get("/foo/bar/baz").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn deeply_nested_inherit_from_middle() {
    let app = Router::new().nest(
        "/foo",
        Router::new()
            .nest("/bar", Router::new())
            .fallback(outer_fallback),
    );

    let client = TestClient::new(app);

    let res = client.get("/foo/bar/baz").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn with_middleware_on_inner_fallback() {
    async fn never_called<B>(_: Request<B>) -> Request<B> {
        panic!("should never be called")
    }

    let inner = Router::new().layer(map_request(never_called));
    let app = Router::new().nest("/foo", inner).fallback(outer_fallback);

    let client = TestClient::new(app);

    let res = client.get("/foo/bar").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn also_inherits_default_layered_fallback() {
    async fn set_header<B>(mut res: Response<B>) -> Response<B> {
        res.headers_mut()
            .insert("x-from-fallback", "1".parse().unwrap());
        res
    }

    let inner = Router::new();
    let app = Router::new()
        .nest("/foo", inner)
        .fallback(outer_fallback)
        .layer(map_response(set_header));

    let client = TestClient::new(app);

    let res = client.get("/foo/bar").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-from-fallback"], "1");
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn nest_fallback_on_inner() {
    let app = Router::new()
        .nest(
            "/foo",
            Router::new()
                .route("/", get(|| async {}))
                .fallback(|| async { (StatusCode::NOT_FOUND, "inner fallback") }),
        )
        .fallback(|| async { (StatusCode::NOT_FOUND, "outer fallback") });

    let client = TestClient::new(app);

    let res = client.get("/foo/not-found").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "inner fallback");
}

// https://github.com/tokio-rs/axum/issues/1931
#[crate::test]
async fn doesnt_panic_if_used_with_nested_router() {
    async fn handler() {}

    let routes_static =
        Router::new().nest_service("/foo", crate::routing::get_service(handler.into_service()));

    let routes_all = Router::new().fallback_service(routes_static);

    let client = TestClient::new(routes_all);

    let res = client.get("/foo/bar").await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[crate::test]
async fn issue_2072() {
    let nested_routes = Router::new().fallback(inner_fallback);

    let app = Router::new()
        .nest("/nested", nested_routes)
        .merge(Router::new());

    let client = TestClient::new(app);

    let res = client.get("/nested/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "inner");

    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");
}

#[crate::test]
async fn issue_2072_outer_fallback_before_merge() {
    let nested_routes = Router::new().fallback(inner_fallback);

    let app = Router::new()
        .nest("/nested", nested_routes)
        .fallback(outer_fallback)
        .merge(Router::new());

    let client = TestClient::new(app);

    let res = client.get("/nested/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "inner");

    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn issue_2072_outer_fallback_after_merge() {
    let nested_routes = Router::new().fallback(inner_fallback);

    let app = Router::new()
        .nest("/nested", nested_routes)
        .merge(Router::new())
        .fallback(outer_fallback);

    let client = TestClient::new(app);

    let res = client.get("/nested/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "inner");

    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn merge_router_with_fallback_into_nested_router_with_fallback() {
    let nested_routes = Router::new().fallback(inner_fallback);

    let app = Router::new()
        .nest("/nested", nested_routes)
        .merge(Router::new().fallback(outer_fallback));

    let client = TestClient::new(app);

    let res = client.get("/nested/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "inner");

    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn merging_nested_router_with_fallback_into_router_with_fallback() {
    let nested_routes = Router::new().fallback(inner_fallback);

    let app = Router::new()
        .fallback(outer_fallback)
        .merge(Router::new().nest("/nested", nested_routes));

    let client = TestClient::new(app);

    let res = client.get("/nested/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "inner");

    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn merge_empty_into_router_with_fallback() {
    let app = Router::new().fallback(outer_fallback).merge(Router::new());

    let client = TestClient::new(app);

    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn merge_router_with_fallback_into_empty() {
    let app = Router::new().merge(Router::new().fallback(outer_fallback));

    let client = TestClient::new(app);

    let res = client.get("/does-not-exist").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "outer");
}

#[crate::test]
async fn mna_fallback_not_405() {
    let app = Router::new()
        .route("/path", get(|| async { "path" }))
        .method_not_allowed_fallback(|| async { (http::StatusCode::NOT_FOUND, "Not Found") });

    let client = TestClient::new(app);
    let method_not_allowed_fallback = client.post("/path").await;

    assert_eq!(
        method_not_allowed_fallback.status(),
        http::StatusCode::NOT_FOUND
    );
    assert_eq!(method_not_allowed_fallback.headers().get(ALLOW), None);
    assert_eq!(method_not_allowed_fallback.text().await, "Not Found");
}

#[crate::test]
async fn mna_fallback_with_existing_fallback() {
    let app = Router::new()
        .route(
            "/",
            get(|| async { "test" }).fallback(|| async { "index fallback" }),
        )
        .route("/path", get(|| async { "path" }))
        .method_not_allowed_fallback(|| async { "method not allowed fallback" });

    let client = TestClient::new(app);
    let index_fallback = client.post("/").await;
    let method_not_allowed_fallback = client.post("/path").await;

    assert_eq!(index_fallback.text().await, "index fallback");
    assert_eq!(
        method_not_allowed_fallback.text().await,
        "method not allowed fallback"
    );
}

#[crate::test]
async fn mna_fallback_with_state() {
    let app = Router::new()
        .route("/", get(|| async { "index" }))
        .method_not_allowed_fallback(|State(state): State<&'static str>| async move { state })
        .with_state("state");

    let client = TestClient::new(app);
    let res = client.post("/").await;
    assert_eq!(res.text().await, "state");
}

#[crate::test]
async fn mna_fallback_with_unused_state() {
    let app = Router::new()
        .route("/", get(|| async { "index" }))
        .with_state(())
        .method_not_allowed_fallback(|| async move { "bla" });

    let client = TestClient::new(app);
    let res = client.post("/").await;
    assert_eq!(res.text().await, "bla");
}

#[crate::test]
async fn state_isnt_cloned_too_much_with_fallback() {
    let state = CountingCloneableState::new();

    let app = Router::new()
        .fallback(|_: State<CountingCloneableState>| async {})
        .with_state(state.clone());

    let client = TestClient::new(app);

    // ignore clones made during setup
    state.setup_done();

    client.get("/does-not-exist").await;

    assert_eq!(state.count(), 3);
}

fn with_layer_test_fallback(router: Router, service: bool) -> Router {
    if service {
        router.fallback_service(service_fn(|_: Request| async {
            Ok::<_, Infallible>((StatusCode::NOT_FOUND, "router fallback"))
        }))
    } else {
        router.fallback(|| async { (StatusCode::NOT_FOUND, "router fallback") })
    }
}

#[allow(deprecated)]
#[crate::test]
async fn route_layer_fallback_bypasses_authentication() {
    for service in [false, true] {
        let app = with_layer_test_fallback(
            Router::new().route("/known", get(|| async { "known" })),
            service,
        )
        .route_layer(ValidateRequestHeaderLayer::bearer("password"));
        let client = TestClient::new(app);

        assert_eq!(
            client.get("/known").await.status(),
            StatusCode::UNAUTHORIZED
        );
        for path in ["/", "/missing"] {
            let res = client.get(path).await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
            assert_eq!(res.text().await, "router fallback");
        }
    }
}

#[allow(deprecated)]
#[crate::test]
async fn route_layer_fallback_keeps_explicit_root_protected() {
    for (service, fallback_first) in [(false, false), (true, false), (false, true)] {
        let root = get(|| async { "root" });
        let app = if fallback_first {
            with_layer_test_fallback(Router::new(), service).route("/", root)
        } else {
            with_layer_test_fallback(Router::new().route("/", root), service)
        }
        .route_layer(ValidateRequestHeaderLayer::bearer("password"));
        let client = TestClient::new(app);

        assert_eq!(client.get("/").await.status(), StatusCode::UNAUTHORIZED);
        let res = client.get("/missing").await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert_eq!(res.text().await, "router fallback");
    }
}

#[allow(deprecated)]
#[tokio::test]
async fn route_layer_fallback_bypasses_authentication_after_merge_and_nest() {
    for service in [false, true] {
        for fallback_first in [false, true] {
            for nested in [false, true] {
                let routes = Router::new().route("/known", get(|| async { "known" }));
                let fallback = with_layer_test_fallback(Router::new(), service);
                let app = if fallback_first {
                    fallback.merge(routes)
                } else {
                    routes.merge(fallback)
                };
                let (app, known, missing) = if nested {
                    (
                        Router::new().nest("/api", app),
                        "/api/known",
                        ["/api", "/api/missing"],
                    )
                } else {
                    (app, "/known", ["/", "/missing"])
                };
                let client = TestClient::new(
                    app.route_layer(ValidateRequestHeaderLayer::bearer("password")),
                );

                assert_eq!(client.get(known).await.status(), StatusCode::UNAUTHORIZED);
                for path in missing {
                    let res = client.get(path).await;
                    assert_eq!(res.status(), StatusCode::NOT_FOUND);
                    assert_eq!(res.text().await, "router fallback");
                }
            }
        }
    }
}

#[allow(deprecated)]
#[crate::test]
async fn route_layer_fallback_full_layer_still_wraps_fallback() {
    for service in [false, true] {
        let app = with_layer_test_fallback(
            Router::new().route("/known", get(|| async { "known" })),
            service,
        )
        .layer(ValidateRequestHeaderLayer::bearer("password"));
        let client = TestClient::new(app);

        for path in ["/known", "/", "/missing"] {
            assert_eq!(client.get(path).await.status(), StatusCode::UNAUTHORIZED);
        }
    }
}

#[allow(deprecated)]
#[crate::test]
async fn route_layer_fallback_keeps_method_fallback_protected() {
    let app = with_layer_test_fallback(
        Router::new().route(
            "/known",
            get(|| async { "known" })
                .fallback(|| async { (StatusCode::IM_A_TEAPOT, "method fallback") }),
        ),
        false,
    )
    .route_layer(ValidateRequestHeaderLayer::bearer("password"));
    let client = TestClient::new(app);

    assert_eq!(
        client.get("/known").await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client.post("/known").await.status(),
        StatusCode::UNAUTHORIZED
    );
    let res = client.get("/missing").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "router fallback");
}

#[allow(deprecated)]
#[test]
fn route_layer_fallback_only_router_has_no_routes() {
    for service in [false, true] {
        let app = with_layer_test_fallback(Router::new(), service);
        assert!(!app.has_routes());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app.route_layer(ValidateRequestHeaderLayer::bearer("password"))
        }));
        assert!(result.is_err());
    }
}

#[allow(deprecated)]
#[tokio::test]
async fn route_layer_fallback_preserves_mixed_root_through_composition() {
    async fn set_header(mut res: Response) -> Response {
        res.headers_mut()
            .insert("x-global", "present".parse().unwrap());
        res
    }

    let inner = Router::new()
        .fallback(|State(state): State<&'static str>| async move {
            (StatusCode::NOT_FOUND, format!("fallback: {state}"))
        })
        .route(
            "/",
            get(|State(state): State<&'static str>| async move { state }),
        )
        .layer(map_response(set_header))
        .with_state("state");
    let merged = Router::new().merge(inner.clone());
    let app = Router::new()
        .nest("/api", merged)
        .route_layer(ValidateRequestHeaderLayer::bearer("password"));
    let client = TestClient::new(app);

    assert_eq!(client.get("/api").await.status(), StatusCode::UNAUTHORIZED);
    let res = client.get("/api/missing").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-global"], "present");
    assert_eq!(res.text().await, "fallback: state");

    let original = TestClient::new(inner);
    assert_eq!(original.get("/").await.text().await, "state");
}

#[allow(deprecated)]
#[crate::test]
async fn route_layer_fallback_allows_empty_method_router() {
    let app = with_layer_test_fallback(Router::new(), false)
        .route("/", crate::routing::MethodRouter::new())
        .route("/known", get(|| async { "known" }))
        .route_layer(ValidateRequestHeaderLayer::bearer("password"));
    let client = TestClient::new(app);

    assert_eq!(
        client.get("/known").await.status(),
        StatusCode::UNAUTHORIZED
    );
    for path in ["/", "/missing"] {
        let res = client.get(path).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert_eq!(res.text().await, "router fallback");
    }
}
