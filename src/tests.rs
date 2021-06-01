use crate::{app, extract};
use http::{Request, Response, StatusCode};
use hyper::{Body, Server};
use serde::Deserialize;
use serde_json::json;
use std::net::{SocketAddr, TcpListener};
use tower::{make::Shared, BoxError, Service};

#[tokio::test]
async fn hello_world() {
    let app = app()
        .at("/")
        .get(|_: Request<Body>| async { "Hello, World!" })
        .into_service();

    let addr = run_in_background(app).await;

    let res = reqwest::get(format!("http://{}", addr)).await.unwrap();
    let body = res.text().await.unwrap();

    assert_eq!(body, "Hello, World!");
}

#[tokio::test]
async fn consume_body() {
    let app = app()
        .at("/")
        .get(|_: Request<Body>, body: String| async { body })
        .into_service();

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

    let app = app()
        .at("/")
        .post(|_: Request<Body>, input: extract::Json<Input>| async { input.into_inner().foo })
        .into_service();

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

    let app = app()
        .at("/")
        .post(|_: Request<Body>, input: extract::Json<Input>| async {
            let input = input.into_inner();
            input.foo
        })
        .into_service();

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

    let app = app()
        .at("/")
        .post(
            |req: Request<Body>, _body: extract::BytesMaxLength<LIMIT>| async move {
                dbg!(&req);
            },
        )
        .into_service();

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
    let app = app()
        .at("/users")
        .get(|_: Request<Body>| async { "users#index" })
        .post(|_: Request<Body>| async { "users#create" })
        .at("/users/:id")
        .get(|_: Request<Body>| async { "users#show" })
        .at("/users/:id/action")
        .get(|_: Request<Body>| async { "users#action" })
        .into_service();

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
    let app = app()
        .at("/users/:id")
        .get(
            |_: Request<Body>, params: extract::UrlParams<(i32,)>| async move {
                let id = params.into_inner();
                assert_eq!(id, 42);
            },
        )
        .post(
            |_: Request<Body>, params_map: extract::UrlParamsMap| async move {
                assert_eq!(params_map.get("id").unwrap(), "1337");
                assert_eq!(params_map.get_typed::<i32>("id").unwrap(), 1337);
            },
        )
        .into_service();

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
async fn boxing() {
    let app = app()
        .at("/")
        .get(|_: Request<Body>| async { "hi from GET" })
        .boxed()
        .post(|_: Request<Body>| async { "hi from POST" })
        .into_service();

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

// TODO(david): tests for adding middleware to single services

// TODO(david): tests for nesting services

/// Run a `tower::Service` in the background and get a URI for it.
pub async fn run_in_background<S, ResBody>(svc: S) -> SocketAddr
where
    S: Service<Request<Body>, Response = Response<ResBody>> + Clone + Send + 'static,
    ResBody: http_body::Body + Send + 'static,
    ResBody::Data: Send,
    ResBody::Error: Into<BoxError>,
    S::Error: Into<BoxError>,
    S::Future: Send,
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
