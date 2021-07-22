use crate::{
    extract::RequestParts, handler::on, prelude::*, response::IntoResponse, routing::MethodFilter,
    service,
};
use bytes::Bytes;
use http::{header::AUTHORIZATION, Request, Response, StatusCode};
use hyper::{Body, Server};
use serde::Deserialize;
use serde_json::json;
use std::{
    convert::Infallible,
    net::{SocketAddr, TcpListener},
    time::Duration,
};
use tower::{make::Shared, service_fn, BoxError, Service, ServiceBuilder};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

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
        get(|params: extract::UrlParams<(i32,)>| async move {
            let (id,) = params.0;
            assert_eq!(id, 42);
        })
        .post(|params_map: extract::UrlParamsMap| async move {
            assert_eq!(params_map.get("id").unwrap(), "1337");
            assert_eq!(
                params_map
                    .get_typed::<i32>("id")
                    .expect("missing")
                    .expect("failed to parse"),
                1337
            );
        }),
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
        get(
            |_: extract::UrlParams<(i32,)>,
             _: extract::UrlParamsMap,
             _: extract::UrlParams<(i32,)>,
             _: extract::UrlParamsMap| async {},
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
}

#[tokio::test]
async fn boxing() {
    let app = route(
        "/",
        on(MethodFilter::Get, |_: Request<Body>| async {
            "hi from GET"
        })
        .on(MethodFilter::Post, |_: Request<Body>| async {
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
async fn service_handlers() {
    use crate::service::ServiceExt as _;
    use tower_http::services::ServeFile;

    let app = route(
        "/echo",
        service::post(
            service_fn(|req: Request<Body>| async move {
                Ok::<_, BoxError>(Response::new(req.into_body()))
            })
            .handle_error(|_error: BoxError| Ok(StatusCode::INTERNAL_SERVER_ERROR)),
        ),
    )
    .route(
        "/static/Cargo.toml",
        service::on(
            MethodFilter::Get,
            ServeFile::new("Cargo.toml").handle_error(|error: std::io::Error| {
                Ok::<_, Infallible>((StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
            }),
        ),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .post(format!("http://{}/echo", addr))
        .body("foobar")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "foobar");

    let res = client
        .get(format!("http://{}/static/Cargo.toml", addr))
        .body("foobar")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.text().await.unwrap().contains("edition ="));
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
            MethodFilter::Put,
            service_fn(|_: Request<Body>| async {
                Ok::<_, Infallible>(Response::new(Body::from("one put")))
            }),
        ),
    )
    .route(
        "/two",
        service::on(MethodFilter::Get, handle.into_service()),
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
async fn handling_errors_from_layered_single_routes() {
    async fn handle(_req: Request<Body>) -> &'static str {
        tokio::time::sleep(Duration::from_secs(10)).await;
        ""
    }

    let app = route(
        "/",
        get(handle
            .layer(
                ServiceBuilder::new()
                    .timeout(Duration::from_millis(100))
                    .layer(TraceLayer::new_for_http())
                    .into_inner(),
            )
            .handle_error(|_error: BoxError| {
                Ok::<_, Infallible>(StatusCode::INTERNAL_SERVER_ERROR)
            })),
    );

    let addr = run_in_background(app).await;

    let res = reqwest::get(format!("http://{}", addr)).await.unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn layer_on_whole_router() {
    async fn handle(_req: Request<Body>) -> &'static str {
        tokio::time::sleep(Duration::from_secs(10)).await;
        ""
    }

    let app = route("/", get(handle))
        .layer(
            ServiceBuilder::new()
                .layer(CompressionLayer::new())
                .timeout(Duration::from_millis(100))
                .into_inner(),
        )
        .handle_error(|_err: BoxError| Ok::<_, Infallible>(StatusCode::INTERNAL_SERVER_ERROR));

    let addr = run_in_background(app).await;

    let res = reqwest::get(format!("http://{}", addr)).await.unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn disjunction() {
    let api_routes = route(
        "/users",
        get(|| async { "users#index" }).post(|| async { "users#create" }),
    )
    .route(
        "/users/:id",
        get(|params: extract::UrlParamsMap| async move {
            format!(
                "{}: users#show ({})",
                params.get("version").unwrap(),
                params.get("id").unwrap()
            )
        }),
    )
    .route(
        "/games/:id",
        get(|params: extract::UrlParamsMap| async move {
            format!(
                "{}: games#show ({})",
                params.get("version").unwrap(),
                params.get("id").unwrap()
            )
        }),
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
async fn typed_header() {
    use extract::TypedHeader;
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

/// Run a `tower::Service` in the background and get a URI for it.
async fn run_in_background<S, ResBody>(svc: S) -> SocketAddr
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
