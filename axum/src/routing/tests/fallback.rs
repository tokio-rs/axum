use super::*;

#[tokio::test]
async fn basic() {
    let app = Router::new()
        .route("/foo", get(|| async {}))
        .fallback(|| async { "fallback" });

    let client = TestClient::new(app);

    assert_eq!(client.get("/foo").send().await.status(), StatusCode::OK);

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "fallback");
}

#[tokio::test]
async fn nest() {
    let app = Router::new()
        .nest(
            "/foo",
            Router::new().route("/bar", get(|| async {})).into_service(),
        )
        .fallback(|| async { "fallback" });

    let client = TestClient::new(app);

    assert_eq!(client.get("/foo/bar").send().await.status(), StatusCode::OK);

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "fallback");
}

#[tokio::test]
async fn or() {
    let one = Router::new().route("/one", get(|| async {}));
    let two = Router::new().route("/two", get(|| async {}));

    let app = one.merge(two).fallback(|| async { "fallback" });

    let client = TestClient::new(app);

    assert_eq!(client.get("/one").send().await.status(), StatusCode::OK);
    assert_eq!(client.get("/two").send().await.status(), StatusCode::OK);

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "fallback");
}

#[tokio::test]
async fn fallback_accessing_state() {
    let app = Router::with_state("state")
        .fallback(|State(state): State<&'static str>| async move { state });

    let client = TestClient::new(app);

    let res = client.get("/does-not-exist").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "state");
}
