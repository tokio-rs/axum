#![allow(clippy::blacklisted_name)]

use crate::{
    extract::RequestParts, handler::on, prelude::*, routing::nest, routing::MethodFilter, service,
};
use bytes::Bytes;
use futures_util::future::Ready;
use http::{header::AUTHORIZATION, Request, Response, StatusCode, Uri};
use hyper::{Body, Server};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashMap,
    convert::Infallible,
    net::{SocketAddr, TcpListener},
    task::{Context, Poll},
    time::Duration,
};
use tower::{make::Shared, service_fn, BoxError, Service};

mod handle_error;
mod nest;
mod or;

#[tokio::test]
async fn hello_world() {
    async fn root(_: Request<Body>) -> &'static str {
        "Hello, World!"
    }

    async fn foo(_: Request<Body>) -> &'static str {
        "foo"
    }

    async fn users_create(_: Request<Body>) -> &'static str {
        "users#create"
    }

    let app = route("/", get(root).post(foo)).route("/users", post(users_create));

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client.get(format!("http://{}", addr)).send().await.unwrap();
    let body = res.text().await.unwrap();
    assert_eq!(body, "Hello, World!");

    let res = client
        .post(format!("http://{}", addr))
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();
    assert_eq!(body, "foo");

    let res = client
        .post(format!("http://{}/users", addr))
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();
    assert_eq!(body, "users#create");
}

#[tokio::test]
async fn consume_body() {
    let app = route("/", get(|body: String| async { body }));

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();
    let res = client
        .get(format!("http://{}", addr))
        .body("foo")
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();

    assert_eq!(body, "foo");
}

#[tokio::test]
async fn deserialize_body() {
    #[derive(Debug, Deserialize)]
    struct Input {
        foo: String,
    }

    let app = route(
        "/",
        post(|input: extract::Json<Input>| async { input.0.foo }),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();
    let res = client
        .post(format!("http://{}", addr))
        .json(&json!({ "foo": "bar" }))
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();

    assert_eq!(body, "bar");
}

#[tokio::test]
async fn consume_body_to_json_requires_json_content_type() {
    #[derive(Debug, Deserialize)]
    struct Input {
        foo: String,
    }

    let app = route(
        "/",
        post(|input: extract::Json<Input>| async { input.0.foo }),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();
    let res = client
        .post(format!("http://{}", addr))
        .body(r#"{ "foo": "bar" }"#)
        .send()
        .await
        .unwrap();

    let status = res.status();
    dbg!(res.text().await.unwrap());

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn body_with_length_limit() {
    use std::iter::repeat;

    #[derive(Debug, Deserialize)]
    struct Input {
        foo: String,
    }

    const LIMIT: u64 = 8;

    let app = route(
        "/",
        post(|_body: extract::ContentLengthLimit<Bytes, LIMIT>| async {}),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .post(format!("http://{}", addr))
        .body(repeat(0_u8).take((LIMIT - 1) as usize).collect::<Vec<_>>())
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .post(format!("http://{}", addr))
        .body(repeat(0_u8).take(LIMIT as usize).collect::<Vec<_>>())
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .post(format!("http://{}", addr))
        .body(repeat(0_u8).take((LIMIT + 1) as usize).collect::<Vec<_>>())
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let res = client
        .post(format!("http://{}", addr))
        .body(reqwest::Body::wrap_stream(futures_util::stream::iter(
            vec![Ok::<_, std::io::Error>(bytes::Bytes::new())],
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::LENGTH_REQUIRED);
}

#[tokio::test]
async fn routing() {
    let app = route(
        "/users",
        get(|_: Request<Body>| async { "users#index" })
            .post(|_: Request<Body>| async { "users#create" }),
    )
    .route("/users/:id", get(|_: Request<Body>| async { "users#show" }))
    .route(
        "/users/:id/action",
        get(|_: Request<Body>| async { "users#action" }),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client.get(format!("http://{}", addr)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = client
        .get(format!("http://{}/users", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "users#index");

    let res = client
        .post(format!("http://{}/users", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "users#create");

    let res = client
        .get(format!("http://{}/users/1", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "users#show");

    let res = client
        .get(format!("http://{}/users/1/action", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "users#action");
}

#[tokio::test]
async fn extracting_url_params() {
    let app = route(
        "/users/:id",
        get(|extract::Path(id): extract::Path<i32>| async move {
            assert_eq!(id, 42);
        })
        .post(
            |extract::Path(params_map): extract::Path<HashMap<String, i32>>| async move {
                assert_eq!(params_map.get("id").unwrap(), &1337);
            },
        ),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/users/42", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .post(format!("http://{}/users/1337", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn extracting_url_params_multiple_times() {
    let app = route(
        "/users/:id",
        get(|_: extract::Path<i32>, _: extract::Path<String>| async {}),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/users/42", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn boxing() {
    let app = route(
        "/",
        on(MethodFilter::GET, |_: Request<Body>| async {
            "hi from GET"
        })
        .on(MethodFilter::POST, |_: Request<Body>| async {
            "hi from POST"
        }),
    )
    .layer(tower_http::compression::CompressionLayer::new())
    .boxed();

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client.get(format!("http://{}", addr)).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "hi from GET");

    let res = client
        .post(format!("http://{}", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "hi from POST");
}

#[tokio::test]
async fn routing_between_services() {
    use std::convert::Infallible;
    use tower::service_fn;

    async fn handle(_: Request<Body>) -> &'static str {
        "handler"
    }

    let app = route(
        "/one",
        service::get(service_fn(|_: Request<Body>| async {
            Ok::<_, Infallible>(Response::new(Body::from("one get")))
        }))
        .post(service_fn(|_: Request<Body>| async {
            Ok::<_, Infallible>(Response::new(Body::from("one post")))
        }))
        .on(
            MethodFilter::PUT,
            service_fn(|_: Request<Body>| async {
                Ok::<_, Infallible>(Response::new(Body::from("one put")))
            }),
        ),
    )
    .route(
        "/two",
        service::on(MethodFilter::GET, handle.into_service()),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/one", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "one get");

    let res = client
        .post(format!("http://{}/one", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "one post");

    let res = client
        .put(format!("http://{}/one", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "one put");

    let res = client
        .get(format!("http://{}/two", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "handler");
}

#[tokio::test]
async fn middleware_on_single_route() {
    use tower::ServiceBuilder;
    use tower_http::{compression::CompressionLayer, trace::TraceLayer};

    async fn handle(_: Request<Body>) -> &'static str {
        "Hello, World!"
    }

    let app = route(
        "/",
        get(handle.layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .into_inner(),
        )),
    );

    let addr = run_in_background(app).await;

    let res = reqwest::get(format!("http://{}", addr)).await.unwrap();
    let body = res.text().await.unwrap();

    assert_eq!(body, "Hello, World!");
}

#[tokio::test]
#[cfg(feature = "header")]
async fn typed_header() {
    use crate::{extract::TypedHeader, response::IntoResponse};

    async fn handle(TypedHeader(user_agent): TypedHeader<headers::UserAgent>) -> impl IntoResponse {
        user_agent.to_string()
    }

    let app = route("/", get(handle));

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}", addr))
        .header("user-agent", "foobar")
        .send()
        .await
        .unwrap();
    let body = res.text().await.unwrap();
    assert_eq!(body, "foobar");

    let res = client.get(format!("http://{}", addr)).send().await.unwrap();
    let body = res.text().await.unwrap();
    assert_eq!(body, "invalid HTTP header (user-agent)");
}

#[tokio::test]
async fn service_in_bottom() {
    async fn handler(_req: Request<hyper::Body>) -> Result<Response<hyper::Body>, hyper::Error> {
        Ok(Response::new(hyper::Body::empty()))
    }

    let app = route("/", service::get(service_fn(handler)));

    run_in_background(app).await;
}

#[tokio::test]
async fn test_extractor_middleware() {
    struct RequireAuth;

    #[async_trait::async_trait]
    impl<B> extract::FromRequest<B> for RequireAuth
    where
        B: Send,
    {
        type Rejection = StatusCode;

        async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
            if let Some(auth) = req
                .headers()
                .expect("headers already extracted")
                .get("authorization")
                .and_then(|v| v.to_str().ok())
            {
                if auth == "secret" {
                    return Ok(Self);
                }
            }

            Err(StatusCode::UNAUTHORIZED)
        }
    }

    async fn handler() {}

    let app = route(
        "/",
        get(handler.layer(extract::extractor_middleware::<RequireAuth>())),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = client
        .get(format!("http://{}/", addr))
        .header(AUTHORIZATION, "secret")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn wrong_method_handler() {
    let app = route("/", get(|| async {}).post(|| async {})).route("/foo", patch(|| async {}));

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .patch(format!("http://{}", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);

    let res = client
        .patch(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .post(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);

    let res = client
        .get(format!("http://{}/bar", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn wrong_method_service() {
    #[derive(Clone)]
    struct Svc;

    impl<R> Service<R> for Svc {
        type Response = Response<http_body::Empty<Bytes>>;
        type Error = Infallible;
        type Future = Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: R) -> Self::Future {
            futures_util::future::ok(Response::new(http_body::Empty::new()))
        }
    }

    let app = route("/", service::get(Svc).post(Svc)).route("/foo", service::patch(Svc));

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .patch(format!("http://{}", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);

    let res = client
        .patch(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .post(format!("http://{}/foo", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);

    let res = client
        .get(format!("http://{}/bar", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Run a `tower::Service` in the background and get a URI for it.
pub(crate) async fn run_in_background<S, ResBody>(svc: S) -> SocketAddr
where
    S: Service<Request<Body>, Response = Response<ResBody>> + Clone + Send + 'static,
    ResBody: http_body::Body + Send + 'static,
    ResBody::Data: Send,
    ResBody::Error: Into<BoxError>,
    S::Future: Send,
    S::Error: Into<BoxError>,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("Could not bind ephemeral socket");
    let addr = listener.local_addr().unwrap();
    println!("Listening on {}", addr);

    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        let server = Server::from_tcp(listener).unwrap().serve(Shared::new(svc));
        tx.send(()).unwrap();
        server.await.expect("server error");
    });

    rx.await.unwrap();

    addr
}
