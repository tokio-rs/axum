use tower::{limit::ConcurrencyLimitLayer, timeout::TimeoutLayer};

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

// TODO(david): can we make this not compile?
// #[tokio::test]
// async fn foo() {
//     let svc_one = service_fn(|_: Request<Body>| async {
//         Ok::<_, hyper::Error>(Response::new(Body::empty()))
//     })
//     .handle_error::<_, _, hyper::Error>(|_| Ok(StatusCode::INTERNAL_SERVER_ERROR));

//     let svc_two = svc_one.clone();

//     let app = svc_one.or(svc_two);

//     let addr = run_in_background(app).await;
// }
