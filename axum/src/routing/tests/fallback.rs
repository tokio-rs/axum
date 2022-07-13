use std::str::FromStr;

use super::*;
use crate::handler::Handler;
use http::header::{HeaderName, HeaderValue};
use tower_http::set_header::SetResponseHeaderLayer;

async fn fallback() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "fallback")
}

async fn api_fallback() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "api fallback")
}

#[tokio::test]
async fn default_fallback() {
    let app = Router::new();

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");
}

#[test]
#[should_panic = "Cannot set fallback twice"]
fn setting_fallback_twice() {
    let _: Router = Router::new()
        .fallback(fallback.into_service())
        .fallback(fallback.into_service());
}

#[tokio::test]
async fn default_fallback_layered() {
    let app = Router::new()
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-1").unwrap(),
            HeaderValue::from_str("1").unwrap(),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-2").unwrap(),
            HeaderValue::from_str("2").unwrap(),
        ));

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert_eq!(res.headers()["x-fallback-2"], "2");
    assert_eq!(res.text().await, "");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert_eq!(res.headers()["x-fallback-2"], "2");
    assert_eq!(res.text().await, "");
}

#[tokio::test]
async fn custom_fallback() {
    let app = Router::new().fallback(fallback.into_service());

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");
}

#[tokio::test]
async fn custom_fallback_with_routes() {
    let app = Router::new()
        .route("/", get(|| async {}))
        .route("/foo", get(|| async {}))
        .fallback(fallback.into_service());

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/foo").send().await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");
}

#[tokio::test]
async fn custom_fallback_layered() {
    let app = Router::new()
        .fallback(fallback.into_service())
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-1").unwrap(),
            HeaderValue::from_str("1").unwrap(),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-2").unwrap(),
            HeaderValue::from_str("2").unwrap(),
        ));

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert_eq!(res.headers()["x-fallback-2"], "2");
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert_eq!(res.headers()["x-fallback-2"], "2");
    assert_eq!(res.text().await, "fallback");
}

#[tokio::test]
async fn merge_default_fallbacks() {
    let app = Router::new().merge(Router::new());

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");
}

#[tokio::test]
async fn merge_default_fallbacks_left_layered() {
    let app = Router::new()
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-1").unwrap(),
            HeaderValue::from_str("1").unwrap(),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-2").unwrap(),
            HeaderValue::from_str("2").unwrap(),
        ))
        .merge(Router::new());

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert_eq!(res.headers()["x-fallback-2"], "2");
    assert_eq!(res.text().await, "");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert_eq!(res.headers()["x-fallback-2"], "2");
    assert_eq!(res.text().await, "");
}

#[tokio::test]
async fn merge_default_fallbacks_right_layered() {
    let app = Router::new().merge(
        Router::new()
            .layer(SetResponseHeaderLayer::overriding(
                HeaderName::from_str("x-fallback-1").unwrap(),
                HeaderValue::from_str("1").unwrap(),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                HeaderName::from_str("x-fallback-2").unwrap(),
                HeaderValue::from_str("2").unwrap(),
            )),
    );

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(res.headers().get("x-fallback-1").is_none());
    assert!(res.headers().get("x-fallback-2").is_none());
    assert_eq!(res.text().await, "");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(res.headers().get("x-fallback-1").is_none());
    assert!(res.headers().get("x-fallback-2").is_none());
    assert_eq!(res.text().await, "");
}

#[tokio::test]
async fn merge_default_fallbacks_both_layered() {
    let app = Router::new()
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-1").unwrap(),
            HeaderValue::from_str("1").unwrap(),
        ))
        .merge(Router::new().layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-2").unwrap(),
            HeaderValue::from_str("2").unwrap(),
        )));

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert!(res.headers().get("x-fallback-2").is_none());
    assert_eq!(res.text().await, "");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert!(res.headers().get("x-fallback-2").is_none());
    assert_eq!(res.text().await, "");
}

#[tokio::test]
async fn merge_custom_fallback_left() {
    let app = Router::new()
        .fallback(fallback.into_service())
        .merge(Router::new());

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");
}

#[tokio::test]
async fn merge_custom_fallback_right() {
    let app = Router::new().merge(Router::new().fallback(fallback.into_service()));

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");
}

#[test]
#[should_panic = "Cannot merge routers that both have fallbacks"]
fn merge_custom_fallback_both() {
    let _: Router = Router::new()
        .fallback(fallback.into_service())
        .merge(Router::new().fallback(fallback.into_service()));
}

#[tokio::test]
async fn nest_default_fallbacks_both() {
    let api = Router::new().route("/users", get(|| async {}));
    let app = Router::new().nest("/api", api);

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    let res = client.get("/api").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    let res = client.get("/api/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    let res = client.get("/api/foo").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    let res = client.get("/api/users").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn nest_default_fallbacks_outer_layered() {
    let api = Router::new().route("/users", get(|| async {}));
    let app = Router::new()
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-1").unwrap(),
            HeaderValue::from_str("1").unwrap(),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-2").unwrap(),
            HeaderValue::from_str("2").unwrap(),
        ))
        .nest("/api", api);

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert_eq!(res.headers()["x-fallback-2"], "2");
    assert_eq!(res.text().await, "");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert_eq!(res.headers()["x-fallback-2"], "2");
    assert_eq!(res.text().await, "");

    let res = client.get("/api").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert_eq!(res.headers()["x-fallback-2"], "2");
    assert_eq!(res.text().await, "");

    let res = client.get("/api/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert_eq!(res.headers()["x-fallback-2"], "2");
    assert_eq!(res.text().await, "");

    let res = client.get("/api/foo").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()["x-fallback-1"], "1");
    assert_eq!(res.headers()["x-fallback-2"], "2");
    assert_eq!(res.text().await, "");

    let res = client.get("/api/users").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn nest_default_fallbacks_inner_layered() {
    let api = Router::new()
        .route("/users", get(|| async {}))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-1").unwrap(),
            HeaderValue::from_str("1").unwrap(),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_str("x-fallback-2").unwrap(),
            HeaderValue::from_str("2").unwrap(),
        ));
    let app = Router::new().nest("/api", api);

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(res.headers().get("x-fallback-1").is_none());
    assert!(res.headers().get("x-fallback-2").is_none());
    assert_eq!(res.text().await, "");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(res.headers().get("x-fallback-1").is_none());
    assert!(res.headers().get("x-fallback-2").is_none());
    assert_eq!(res.text().await, "");

    let res = client.get("/api").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(res.headers().get("x-fallback-1").is_none());
    assert!(res.headers().get("x-fallback-2").is_none());
    assert_eq!(res.text().await, "");

    let res = client.get("/api/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(res.headers().get("x-fallback-1").is_none());
    assert!(res.headers().get("x-fallback-2").is_none());
    assert_eq!(res.text().await, "");

    let res = client.get("/api/foo").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(res.headers().get("x-fallback-1").is_none());
    assert!(res.headers().get("x-fallback-2").is_none());
    assert_eq!(res.text().await, "");

    let res = client.get("/api/users").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn nest_custom_fallbacks_outer() {
    let api = Router::new().route("/users", get(|| async {}));
    let app = Router::new()
        .fallback(fallback.into_service())
        .nest("/api", api);

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/api").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/api/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/api/foo").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/api/users").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn nest_custom_fallbacks_inner() {
    let api = Router::new()
        .fallback(api_fallback.into_service())
        .route("/users", get(|| async {}));
    let app = Router::new().nest("/api", api);

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    let res = client.get("/api").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "api fallback");

    let res = client.get("/api/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "api fallback");

    let res = client.get("/api/foo").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "api fallback");

    let res = client.get("/api/users").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn nest_with_trailing_slash_custom_fallbacks_inner() {
    let api = Router::new()
        .fallback(api_fallback.into_service())
        .route("/users", get(|| async {}));
    let app = Router::new().nest("/api/", api);

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    // this one goes the outer fallback since we nested at `/api/`
    let res = client.get("/api").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "");

    let res = client.get("/api/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "api fallback");

    let res = client.get("/api/foo").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "api fallback");

    let res = client.get("/api/users").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn nest_custom_fallbacks_both() {
    let api = Router::new()
        .fallback(api_fallback.into_service())
        .route("/users", get(|| async {}));
    let app = Router::new()
        .fallback(fallback.into_service())
        .nest("/api", api);

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/api").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "api fallback");

    let res = client.get("/api/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "api fallback");

    let res = client.get("/api/foo").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "api fallback");

    let res = client.get("/api/users").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn nest_custom_fallbacks_both_add_outer_fallback_last() {
    let api = Router::new()
        .fallback(api_fallback.into_service())
        .route("/users", get(|| async {}));
    let app = Router::new()
        .nest("/api", api)
        .fallback(fallback.into_service());

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");

    let res = client.get("/api").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "api fallback");

    let res = client.get("/api/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "api fallback");

    let res = client.get("/api/foo").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "api fallback");

    let res = client.get("/api/users").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn nested_inner_fallback_sees_url_params() {
    async fn api_fallback(Path(version): Path<u32>) -> impl IntoResponse {
        (StatusCode::NOT_FOUND, format!("{}", version))
    }

    let api = Router::new().fallback(api_fallback.into_service());

    let app = Router::new().nest("/api/:version", api);

    let client = TestClient::new(app);

    let res = client.get("/api/123").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "123");

    let res = client.get("/api/123/").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "123");

    let res = client.get("/api/123/foo").send().await;
    let status = res.status();
    assert_eq!(res.text().await, "123");
    assert_eq!(status, StatusCode::NOT_FOUND);
}
