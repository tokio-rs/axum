use super::*;
use http::Method;
use tower::ServiceExt;

mod for_handlers {
    use super::*;
    use http::HeaderMap;

    #[tokio::test]
    async fn get_handles_head() {
        let app = Router::new().route(
            "/",
            get(|| async {
                let mut headers = HeaderMap::new();
                headers.insert("x-some-header", "foobar".parse().unwrap());
                (headers, "you shouldn't see this")
            }),
        );

        // don't use reqwest because it always strips bodies from HEAD responses
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .method(Method::HEAD)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["x-some-header"], "foobar");

        let body = hyper::body::to_bytes(res.into_body()).await.unwrap();
        assert_eq!(body.len(), 0);
    }
}

mod for_services {
    use super::*;
    use crate::routing::get_service;
    use http::header::HeaderValue;

    #[tokio::test]
    async fn get_handles_head() {
        let app = Router::new().route(
            "/",
            get_service(service_fn(|_req: Request<Body>| async move {
                let res = Response::builder()
                    .header("x-some-header", "foobar".parse::<HeaderValue>().unwrap())
                    .body(Body::from("you shouldn't see this"))
                    .unwrap();
                Ok::<_, Infallible>(res)
            })),
        );

        // don't use reqwest because it always strips bodies from HEAD responses
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .method(Method::HEAD)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["x-some-header"], "foobar");

        let body = hyper::body::to_bytes(res.into_body()).await.unwrap();
        assert_eq!(body.len(), 0);
    }
}
