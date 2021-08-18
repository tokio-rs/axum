use serde_json::{json, Value};
use tower::{limit::ConcurrencyLimitLayer, timeout::TimeoutLayer};

use crate::{extract::OriginalUri, response::IntoResponse, Json};

use super::*;

#[tokio::test]
async fn basic() {
    let one = route("/foo", get(|| async {})).route("/bar", get(|| async {}));
    let two = route("/baz", get(|| async {}));
    let app = one.or(two);

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .get(format!("http://{}/bar", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .get(format!("http://{}/baz", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .get(format!("http://{}/qux", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn multiple_ors_balanced_differently() {
    let one = route("/one", get(|| async { "one" }));
    let two = route("/two", get(|| async { "two" }));
    let three = route("/three", get(|| async { "three" }));
    let four = route("/four", get(|| async { "four" }));

    test(
        "one",
        one.clone()
            .or(two.clone())
            .or(three.clone())
            .or(four.clone()),
    )
    .await;

    test(
        "two",
        one.clone()
            .or(two.clone())
            .or(three.clone().or(four.clone())),
    )
    .await;

    test(
        "three",
        one.clone()
            .or(two.clone().or(three.clone()).or(four.clone())),
    )
    .await;

    test("four", one.or(two.or(three.or(four)))).await;

    async fn test<S, ResBody>(name: &str, app: S)
    where
        S: Service<Request<Body>, Response = Response<ResBody>> + Clone + Send + 'static,
        ResBody: http_body::Body + Send + 'static,
        ResBody::Data: Send,
        ResBody::Error: Into<BoxError>,
        S::Future: Send,
        S::Error: Into<BoxError>,
    {
        let addr = run_in_background(app).await;

        let client = reqwest::Client::new();

        for n in ["one", "two", "three", "four"].iter() {
            println!("running: {} / {}", name, n);
            let res = client
                .get(format!("http://{}/{}", addr, n))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(res.text().await.unwrap(), *n);
        }
    }
}

#[tokio::test]
async fn or_nested_inside_other_thing() {
    let inner = route("/bar", get(|| async {})).or(route("/baz", get(|| async {})));
    let app = nest("/foo", inner);

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/foo/bar", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .get(format!("http://{}/foo/baz", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn or_with_route_following() {
    let one = route("/one", get(|| async { "one" }));
    let two = route("/two", get(|| async { "two" }));
    let app = one.or(two).route("/three", get(|| async { "three" }));

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/one", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .get(format!("http://{}/two", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .get(format!("http://{}/three", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn layer() {
    let one = route("/foo", get(|| async {}));
    let two = route("/bar", get(|| async {})).layer(ConcurrencyLimitLayer::new(10));
    let app = one.or(two);

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .get(format!("http://{}/bar", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn layer_and_handle_error() {
    let one = route("/foo", get(|| async {}));
    let two = route("/time-out", get(futures::future::pending::<()>))
        .layer(TimeoutLayer::new(Duration::from_millis(10)))
        .handle_error(|_| Ok(StatusCode::REQUEST_TIMEOUT));
    let app = one.or(two);

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/time-out", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);
}

#[tokio::test]
async fn nesting() {
    let one = route("/foo", get(|| async {}));
    let two = nest("/bar", route("/baz", get(|| async {})));
    let app = one.or(two);

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/bar/baz", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn boxed() {
    let one = route("/foo", get(|| async {})).boxed();
    let two = route("/bar", get(|| async {})).boxed();
    let app = one.or(two);

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/bar", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn many_ors() {
    let app = route("/r1", get(|| async {}))
        .or(route("/r2", get(|| async {})))
        .or(route("/r3", get(|| async {})))
        .or(route("/r4", get(|| async {})))
        .or(route("/r5", get(|| async {})))
        .or(route("/r6", get(|| async {})))
        .or(route("/r7", get(|| async {})));

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    for n in 1..=7 {
        let res = client
            .get(format!("http://{}/r{}", addr, n))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    let res = client
        .get(format!("http://{}/r8", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn services() {
    let app = route(
        "/foo",
        crate::service::get(service_fn(|_: Request<Body>| async {
            Ok::<_, Infallible>(Response::new(Body::empty()))
        })),
    )
    .or(route(
        "/bar",
        crate::service::get(service_fn(|_: Request<Body>| async {
            Ok::<_, Infallible>(Response::new(Body::empty()))
        })),
    ));

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .get(format!("http://{}/bar", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

async fn all_the_uris(
    uri: Uri,
    OriginalUri(original_uri): OriginalUri,
    req: Request<Body>,
) -> impl IntoResponse {
    Json(json!({
        "uri": uri.to_string(),
        "request_uri": req.uri().to_string(),
        "original_uri": original_uri.to_string(),
    }))
}

#[tokio::test]
async fn nesting_and_seeing_the_right_uri() {
    let one = nest("/foo", route("/bar", get(all_the_uris)));
    let two = route("/foo", get(all_the_uris));

    let addr = run_in_background(one.or(two)).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/foo/bar", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await.unwrap(),
        json!({
            "uri": "/bar",
            "request_uri": "/bar",
            "original_uri": "/foo/bar",
        })
    );

    let res = client
        .get(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await.unwrap(),
        json!({
            "uri": "/foo",
            "request_uri": "/foo",
            "original_uri": "/foo",
        })
    );
}

#[tokio::test]
async fn nesting_and_seeing_the_right_uri_at_more_levels_of_nesting() {
    let one = nest("/foo", nest("/bar", route("/baz", get(all_the_uris))));
    let two = route("/foo", get(all_the_uris));

    let addr = run_in_background(one.or(two)).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/foo/bar/baz", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await.unwrap(),
        json!({
            "uri": "/baz",
            "request_uri": "/baz",
            "original_uri": "/foo/bar/baz",
        })
    );

    let res = client
        .get(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await.unwrap(),
        json!({
            "uri": "/foo",
            "request_uri": "/foo",
            "original_uri": "/foo",
        })
    );
}

#[tokio::test]
async fn nesting_and_seeing_the_right_uri_ors_with_nesting() {
    let one = nest("/foo", nest("/bar", route("/baz", get(all_the_uris))));
    let two = nest("/foo", route("/qux", get(all_the_uris)));
    let three = route("/foo", get(all_the_uris));

    let addr = run_in_background(one.or(two).or(three)).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/foo/bar/baz", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await.unwrap(),
        json!({
            "uri": "/baz",
            "request_uri": "/baz",
            "original_uri": "/foo/bar/baz",
        })
    );

    let res = client
        .get(format!("http://{}/foo/qux", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await.unwrap(),
        json!({
            "uri": "/qux",
            "request_uri": "/qux",
            "original_uri": "/foo/qux",
        })
    );

    let res = client
        .get(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await.unwrap(),
        json!({
            "uri": "/foo",
            "request_uri": "/foo",
            "original_uri": "/foo",
        })
    );
}

#[tokio::test]
async fn nesting_and_seeing_the_right_uri_ors_with_multi_segment_uris() {
    let one = nest("/foo", nest("/bar", route("/baz", get(all_the_uris))));
    let two = route("/foo/bar", get(all_the_uris));

    let addr = run_in_background(one.or(two)).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/foo/bar/baz", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await.unwrap(),
        json!({
            "uri": "/baz",
            "request_uri": "/baz",
            "original_uri": "/foo/bar/baz",
        })
    );

    let res = client
        .get(format!("http://{}/foo/bar", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await.unwrap(),
        json!({
            "uri": "/foo/bar",
            "request_uri": "/foo/bar",
            "original_uri": "/foo/bar",
        })
    );
}
