//! Regression tests for https://github.com/tokio-rs/axum/issues/2536
//!
//! `Router::route_service` takes a [`Service`] whose request body is
//! [`crate::body::Body`] and whose response implements [`IntoResponse`]. It
//! does not require the removed `BoxBody` type, and it matches every HTTP
//! method. `any_service` / `get_service` exist to build a [`MethodRouter`].

use super::*;
use crate::routing::any_service;
use http_body_util::Full;
use tower_http::services::ServeFile;

async fn send(app: Router, method: Method, uri: &str) -> (StatusCode, HeaderMap, Bytes) {
    let res = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (parts, body) = res.into_parts();
    let body = BodyExt::collect(body).await.unwrap().to_bytes();
    (parts.status, parts.headers, body)
}

#[crate::test]
async fn route_service_uses_body_without_boxing() {
    let app = Router::new().route_service(
        "/foo",
        service_fn(|req: Request| async move {
            let body = Body::from(format!("Hi from `{} /foo`", req.method()));
            Ok::<_, Infallible>(Response::new(body))
        }),
    );

    let (status, _, body) = send(app, Method::GET, "/foo").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "Hi from `GET /foo`");
}

#[crate::test]
async fn route_service_accepts_any_http_method() {
    let app = Router::new().route_service(
        "/foo",
        service_fn(|req: Request| async move {
            let body = Body::from(format!("Hi from `{} /foo`", req.method()));
            Ok::<_, Infallible>(Response::new(body))
        }),
    );

    for method in [
        Method::GET,
        Method::HEAD,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::OPTIONS,
        Method::TRACE,
    ] {
        let (status, _, body) = send(app.clone(), method.clone(), "/foo").await;
        assert_eq!(status, StatusCode::OK, "{method} should be forwarded");

        if method == Method::HEAD {
            // The service still produces a body; `route_service` does not strip it.
            assert_eq!(body, "Hi from `HEAD /foo`");
        } else {
            assert_eq!(body, format!("Hi from `{method} /foo`"));
        }
    }
}

#[crate::test]
async fn route_service_maps_non_axum_body_via_into_response() {
    // Previously docs claimed you had to wrap services in `any_service` so the
    // response body could be mapped to `BoxBody`. `IntoResponse` now does that
    // for `route_service` directly.
    let app = Router::new().route_service(
        "/",
        service_fn(|_: Request| async {
            Ok::<_, Infallible>(Response::new(Full::new(Bytes::from("hello"))))
        }),
    );

    let (status, _, body) = send(app, Method::GET, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "hello");
}

#[crate::test]
async fn route_service_accepts_into_response_values() {
    let app = Router::new().route_service(
        "/",
        service_fn(|_: Request| async { Ok::<_, Infallible>("ok") }),
    );

    let (status, _, body) = send(app.clone(), Method::GET, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");

    let (status, _, body) = send(app, Method::POST, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[crate::test]
async fn any_service_via_route_accepts_any_http_method() {
    let app = Router::new().route(
        "/",
        any_service(service_fn(|_: Request| async {
            Ok::<_, Infallible>(Response::new(Body::from("Hi from `/`")))
        })),
    );

    for method in [Method::GET, Method::POST, Method::PUT, Method::DELETE] {
        let (status, _, body) = send(app.clone(), method.clone(), "/").await;
        assert_eq!(status, StatusCode::OK, "{method} should be forwarded");
        assert_eq!(body, "Hi from `/`");
    }
}

#[crate::test]
async fn get_service_via_route_rejects_non_get() {
    let app = Router::new().route(
        "/",
        get_service(service_fn(|_: Request| async {
            Ok::<_, Infallible>(Response::new(Body::from("get")))
        })),
    );

    let (status, _, body) = send(app.clone(), Method::GET, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "get");

    let (status, headers, _) = send(app, Method::POST, "/").await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    let allow = headers.get(ALLOW).expect("Allow header").to_str().unwrap();
    assert!(allow.contains("GET"), "{allow}");
}

#[crate::test]
async fn any_service_can_run_without_a_router() {
    let mut svc = any_service(service_fn(|_: Request| async {
        Ok::<_, Infallible>(Response::new(Body::from("ok")))
    }));

    let res = TowerServiceExt::oneshot(
        &mut svc,
        Request::builder()
            .method(Method::PUT)
            .uri("/")
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
    .into_response();

    assert_eq!(res.status(), StatusCode::OK);
    let body = BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    assert_eq!(body, "ok");
}

#[crate::test]
async fn route_service_can_use_tower_http_file_service() {
    let app = Router::new().route_service("/static/README.md", ServeFile::new("README.md"));

    let (status, _, body) = send(app, Method::GET, "/static/README.md").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[crate::test]
async fn documented_route_service_example() {
    let app = Router::new()
        .route(
            "/",
            any_service(service_fn(|_: Request| async {
                let res = Response::new(Body::from("Hi from `/`"));
                Ok::<_, Infallible>(res)
            })),
        )
        .route_service(
            "/foo",
            service_fn(|req: Request| async move {
                let body = Body::from(format!("Hi from `{} /foo`", req.method()));
                let res = Response::new(body);
                Ok::<_, Infallible>(res)
            }),
        );

    let (status, _, body) = send(app.clone(), Method::GET, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "Hi from `/`");

    let (status, _, body) = send(app.clone(), Method::POST, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "Hi from `/`");

    let (status, _, body) = send(app.clone(), Method::GET, "/foo").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "Hi from `GET /foo`");

    let (status, _, body) = send(app, Method::PUT, "/foo").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "Hi from `PUT /foo`");
}
