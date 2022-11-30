//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-unix-domain-socket
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
        http::{Method, Request, StatusCode, Uri},
        routing::get,
        Router,
    };
    use futures::ready;
    use hyper::{
        client::connect::{Connected, Connection},
        server::accept::Accept,
    };
    use std::{
        io,
        path::PathBuf,
        pin::Pin,
        sync::Arc,
        task::{Context, Poll},
    };
    use tokio::{
        io::{AsyncRead, AsyncWrite},
        net::{unix::UCred, UnixListener, UnixStream},
    };
    use tower::BoxError;
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
        tokio::spawn(async {
            let app = Router::new().route("/", get(handler));

            axum::Server::builder(ServerAccept { uds })
                .serve(app.into_make_service_with_connect_info::<UdsConnectInfo>())
                .await
                .unwrap();
        });

        let connector = tower::service_fn(move |_: Uri| {
            let path = path.clone();
            Box::pin(async move {
                let stream = UnixStream::connect(path).await?;
                Ok::<_, io::Error>(ClientConnection { stream })
            })
        });
        let client = hyper::Client::builder().build(connector);

        let request = Request::builder()
            .method(Method::GET)
            .uri("http://uri-doesnt-matter.com")
            .body(Body::empty())
            .unwrap();

        let response = client.request(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body, "Hello, World!");
    }

    async fn handler(ConnectInfo(info): ConnectInfo<UdsConnectInfo>) -> &'static str {
        println!("new connection from `{:?}`", info);

        "Hello, World!"
    }

    struct ServerAccept {
        uds: UnixListener,
    }

    impl Accept for ServerAccept {
        type Conn = UnixStream;
        type Error = BoxError;

        fn poll_accept(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
            let (stream, _addr) = ready!(self.uds.poll_accept(cx))?;
            Poll::Ready(Some(Ok(stream)))
        }
    }

    struct ClientConnection {
        stream: UnixStream,
    }

    impl AsyncWrite for ClientConnection {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, io::Error>> {
            Pin::new(&mut self.stream).poll_write(cx, buf)
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), io::Error>> {
            Pin::new(&mut self.stream).poll_flush(cx)
        }

        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), io::Error>> {
            Pin::new(&mut self.stream).poll_shutdown(cx)
        }
    }

    impl AsyncRead for ClientConnection {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.stream).poll_read(cx, buf)
        }
    }

    impl Connection for ClientConnection {
        fn connected(&self) -> Connected {
            Connected::new()
        }
    }

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    struct UdsConnectInfo {
        peer_addr: Arc<tokio::net::unix::SocketAddr>,
        peer_cred: UCred,
    }

    impl connect_info::Connected<&UnixStream> for UdsConnectInfo {
        fn connect_info(target: &UnixStream) -> Self {
            let peer_addr = target.peer_addr().unwrap();
            let peer_cred = target.peer_cred().unwrap();

            Self {
                peer_addr: Arc::new(peer_addr),
                peer_cred,
            }
        }
    }
}
