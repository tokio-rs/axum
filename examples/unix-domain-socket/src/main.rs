//! Run with
//!
//! ```not_rust
//! cargo run -p example-unix-domain-socket
//! ```

#[cfg(unix)]
#[tokio::main]
async fn main() {
    unix::server().await;
}

#[cfg(not(unix))]
fn main() {
    println!("This example requires unix")
}

#[cfg(unix)]
mod unix {
    use axum::{
        body::Body,
        extract::connect_info::ConnectInfo,
        http::{Method, Request, StatusCode},
        routing::get,
        Router, UdsConnectInfo,
    };
    use http_body_util::BodyExt;
    use hyper_util::rt::TokioIo;
    use std::path::PathBuf;
    use tokio::net::{UnixListener, UnixStream};
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    pub async fn server() {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "debug".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();

        let path = PathBuf::from("/tmp/axum/helloworld");

        let _ = tokio::fs::remove_file(&path).await;
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();

        let uds = UnixListener::bind(path.clone()).unwrap();

        let app = Router::new().route("/", get(handler));

        tokio::spawn(async move {
            axum::serve(
                uds,
                app.into_make_service_with_connect_info::<UdsConnectInfo>(),
            )
            .await
            .unwrap()
        });

        let stream = TokioIo::new(UnixStream::connect(path).await.unwrap());
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                println!("Connection failed: {:?}", err);
            }
        });

        let request = Request::builder()
            .method(Method::GET)
            .uri("http://uri-doesnt-matter.com")
            .body(Body::empty())
            .unwrap();

        let response = sender.send_request(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.collect().await.unwrap().to_bytes();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body, "Hello, World!");
    }

    async fn handler(ConnectInfo(info): ConnectInfo<UdsConnectInfo>) -> &'static str {
        println!("new connection from `{:?}`", info);

        "Hello, World!"
    }
}
