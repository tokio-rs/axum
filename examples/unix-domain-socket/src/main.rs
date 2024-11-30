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
        extract::connect_info::{self, ConnectInfo},
        http::{Method, Request, StatusCode},
        routing::get,
        serve::IncomingStream,
        Router,
    };
    use http_body_util::BodyExt;
    use hyper_util::rt::TokioIo;
    use std::{path::PathBuf, sync::Arc};
    use tokio::net::{unix::UCred, UnixListener, UnixStream};
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
        tokio::spawn(async move {
            let app = Router::new()
                .route("/", get(handler))
                .into_make_service_with_connect_info::<UdsConnectInfo>();

            axum::serve(uds, app).await.unwrap();
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

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    struct UdsConnectInfo {
        peer_addr: Arc<tokio::net::unix::SocketAddr>,
        peer_cred: UCred,
    }

    impl connect_info::Connected<IncomingStream<'_, UnixListener>> for UdsConnectInfo {
        fn connect_info(stream: IncomingStream<'_, UnixListener>) -> Self {
            let peer_addr = stream.io().peer_addr().unwrap();
            let peer_cred = stream.io().peer_cred().unwrap();
            Self {
                peer_addr: Arc::new(peer_addr),
                peer_cred,
            }
        }
    }
}
