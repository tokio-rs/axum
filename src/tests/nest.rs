use super::*;
use crate::body::box_body;
use std::collections::HashMap;

#[tokio::test]
async fn nesting_apps() {
    let api_routes = route(
        "/users",
        get(|| async { "users#index" }).post(|| async { "users#create" }),
    )
    .route(
        "/users/:id",
        get(
            |params: extract::Path<HashMap<String, String>>| async move {
                format!(
                    "{}: users#show ({})",
                    params.get("version").unwrap(),
                    params.get("id").unwrap()
                )
            },
        ),
    )
    .route(
        "/games/:id",
        get(
            |params: extract::Path<HashMap<String, String>>| async move {
                format!(
                    "{}: games#show ({})",
                    params.get("version").unwrap(),
                    params.get("id").unwrap()
                )
            },
        ),
    );

    let app = route("/", get(|| async { "hi" })).nest("/:version/api", api_routes);

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "hi");

    let res = client
        .get(format!("http://{}/v0/api/users", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "users#index");

    let res = client
        .get(format!("http://{}/v0/api/users/123", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "v0: users#show (123)");

    let res = client
        .get(format!("http://{}/v0/api/games/123", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "v0: games#show (123)");
}

#[tokio::test]
async fn wrong_method_nest() {
    let nested_app = route("/", get(|| async {}));
    let app = crate::routing::nest("/", nested_app);

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client.get(format!("http://{}", addr)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .post(format!("http://{}", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);

    let res = client
        .patch(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn nesting_at_root() {
    let app = nest("/", get(|uri: Uri| async move { uri.to_string() }));

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client.get(format!("http://{}", addr)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "/");

    let res = client
        .get(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "/foo");

    let res = client
        .get(format!("http://{}/foo/bar", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "/foo/bar");
}

#[tokio::test]
async fn nested_url_extractor() {
    let app = nest(
        "/foo",
        nest(
            "/bar",
            route("/baz", get(|uri: Uri| async move { uri.to_string() })).route(
                "/qux",
                get(|req: Request<Body>| async move { req.uri().to_string() }),
            ),
        ),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/foo/bar/baz", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "/foo/bar/baz");

    let res = client
        .get(format!("http://{}/foo/bar/qux", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "/foo/bar/qux");
}

#[tokio::test]
async fn nested_url_nested_extractor() {
    let app = nest(
        "/foo",
        nest(
            "/bar",
            route(
                "/baz",
                get(|uri: extract::NestedUri| async move { uri.0.to_string() }),
            ),
        ),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/foo/bar/baz", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "/baz");
}

#[tokio::test]
async fn nested_service_sees_original_uri() {
    let app = nest(
        "/foo",
        nest(
            "/bar",
            route(
                "/baz",
                service_fn(|req: Request<Body>| async move {
                    let body = box_body(Body::from(req.uri().to_string()));
                    Ok::<_, Infallible>(Response::new(body))
                }),
            ),
        ),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/foo/bar/baz", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "/foo/bar/baz");
}
