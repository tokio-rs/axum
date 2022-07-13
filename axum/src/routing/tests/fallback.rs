use super::*;
use crate::handler::Handler;

async fn fallback() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "fallback")
}

#[tokio::test]
async fn basic() {
    let app = Router::new()
        .route("/", get(|| async {}))
        .route("/foo", get(|| async {}))
        .fallback(fallback.into_service());

    let client = TestClient::new(app);

    assert_eq!(client.get("/").send().await.status(), StatusCode::OK);
    assert_eq!(client.get("/foo").send().await.status(), StatusCode::OK);

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");
}

#[tokio::test]
async fn nest() {
    let app = Router::new()
        .nest("/foo", Router::new().route("/bar", get(|| async {})))
        .fallback(fallback.into_service());

    let client = TestClient::new(app);

    assert_eq!(client.get("/foo/bar").send().await.status(), StatusCode::OK);

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");
}

#[tokio::test]
async fn or() {
    let one = Router::new().route("/one", get(|| async {}));
    let two = Router::new().route("/two", get(|| async {}));

    let app = one.merge(two).fallback(fallback.into_service());

    let client = TestClient::new(app);

    assert_eq!(client.get("/one").send().await.status(), StatusCode::OK);
    assert_eq!(client.get("/two").send().await.status(), StatusCode::OK);

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await, "fallback");
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
