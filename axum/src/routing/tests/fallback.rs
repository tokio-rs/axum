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
        Router::new().nest_service("/", crate::routing::get_service(handler.into_service()));

    let routes_all = Router::new().fallback_service(routes_static);

    let client = TestClient::new(routes_all);

    let res = client.get("/foobar").await;
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
