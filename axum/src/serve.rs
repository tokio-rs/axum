//! Serve services.

pub use tower_serve::{
    serve, ConnLimiter, ConnLimiterIo, ConnectionLifetimeLimits, Executor, IncomingStream,
    Listener, ListenerExt, MaxConnectionAge, Serve, TapIo, TokioExecutor, WithGracefulShutdown,
};

#[cfg(test)]
mod tests {
    use std::{future::IntoFuture as _, net::SocketAddr, time::Duration};

    use axum_core::{body::Body, extract::Request};
    use http::StatusCode;
    use hyper_util::rt::TokioIo;
    #[cfg(unix)]
    use tokio::net::UnixListener;
    use tokio::{
        net::{TcpListener, TcpStream},
        task::JoinHandle,
    };

    #[cfg(unix)]
    use super::IncomingStream;
    use super::{serve, ConnectionLifetimeLimits, ListenerExt, MaxConnectionAge};
    #[cfg(unix)]
    use crate::extract::connect_info::Connected;
    use crate::{
        body::to_bytes,
        handler::{Handler, HandlerWithoutStateExt},
        routing::get,
        Router,
    };

    // Compile-only coverage of every axum make-service integration accepted by
    // `serve` (`Router`, `MethodRouter`, `Handler` and their `into_make_service*`
    // forms), plus `Connected<IncomingStream>` and `ListenerExt::tap_io`. The
    // runtime behaviour of `serve` itself is tested in the `tower-serve` crate; this
    // only guards the axum-specific type plumbing.
    #[allow(dead_code, unused)]
    async fn if_it_compiles_it_works() {
        #[derive(Clone, Debug)]
        struct UdsConnectInfo;

        #[cfg(unix)]
        impl Connected<IncomingStream<'_, UnixListener>> for UdsConnectInfo {
            fn connect_info(_stream: IncomingStream<'_, UnixListener>) -> Self {
                Self
            }
        }

        let router: Router = Router::new();

        let addr = "0.0.0.0:0";

        let tcp_nodelay_listener = || async {
            TcpListener::bind(addr).await.unwrap().tap_io(|tcp_stream| {
                if let Err(err) = tcp_stream.set_nodelay(true) {
                    eprintln!("failed to set TCP_NODELAY on incoming connection: {err:#}");
                }
            })
        };

        // router
        serve(TcpListener::bind(addr).await.unwrap(), router.clone());
        serve(tcp_nodelay_listener().await, router.clone()).await;
        #[cfg(unix)]
        serve(UnixListener::bind("").unwrap(), router.clone());

        serve(
            TcpListener::bind(addr).await.unwrap(),
            router.clone().into_make_service(),
        );
        serve(
            tcp_nodelay_listener().await,
            router.clone().into_make_service(),
        );
        #[cfg(unix)]
        serve(
            UnixListener::bind("").unwrap(),
            router.clone().into_make_service(),
        );

        serve(
            TcpListener::bind(addr).await.unwrap(),
            router
                .clone()
                .into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        serve(
            tcp_nodelay_listener().await,
            router
                .clone()
                .into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        #[cfg(unix)]
        serve(
            UnixListener::bind("").unwrap(),
            router.into_make_service_with_connect_info::<UdsConnectInfo>(),
        );

        // method router
        serve(TcpListener::bind(addr).await.unwrap(), get(handler));
        serve(tcp_nodelay_listener().await, get(handler));
        #[cfg(unix)]
        serve(UnixListener::bind("").unwrap(), get(handler));

        serve(
            TcpListener::bind(addr).await.unwrap(),
            get(handler).into_make_service(),
        );
        serve(
            tcp_nodelay_listener().await,
            get(handler).into_make_service(),
        );
        #[cfg(unix)]
        serve(
            UnixListener::bind("").unwrap(),
            get(handler).into_make_service(),
        );

        serve(
            TcpListener::bind(addr).await.unwrap(),
            get(handler).into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        serve(
            tcp_nodelay_listener().await,
            get(handler).into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        #[cfg(unix)]
        serve(
            UnixListener::bind("").unwrap(),
            get(handler).into_make_service_with_connect_info::<UdsConnectInfo>(),
        );

        // handler
        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_service(),
        );
        serve(tcp_nodelay_listener().await, handler.into_service());
        #[cfg(unix)]
        serve(UnixListener::bind("").unwrap(), handler.into_service());

        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.with_state(()),
        );
        serve(tcp_nodelay_listener().await, handler.with_state(()));
        #[cfg(unix)]
        serve(UnixListener::bind("").unwrap(), handler.with_state(()));

        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_make_service(),
        );
        serve(tcp_nodelay_listener().await, handler.into_make_service());
        #[cfg(unix)]
        serve(UnixListener::bind("").unwrap(), handler.into_make_service());

        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        serve(
            tcp_nodelay_listener().await,
            handler.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        #[cfg(unix)]
        serve(
            UnixListener::bind("").unwrap(),
            handler.into_make_service_with_connect_info::<UdsConnectInfo>(),
        );

        // with_executor
        let router: Router = Router::new();
        let exec = TestExecutor::new();
        serve(TcpListener::bind(addr).await.unwrap(), router.clone()).with_executor(exec.clone());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .with_executor(exec.clone())
            .with_graceful_shutdown(std::future::pending());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .with_graceful_shutdown(std::future::pending())
            .with_executor(exec.clone());
        serve(TcpListener::bind(addr).await.unwrap(), get(handler)).with_executor(exec.clone());
        serve(
            TcpListener::bind(addr).await.unwrap(),
            handler.into_make_service(),
        )
        .with_executor(exec);

        // connection_lifetime_limits, composable with the other builder methods in any order
        let limits = ConnectionLifetimeLimits::new().max_connection_age(
            MaxConnectionAge::new(Duration::from_secs(60))
                .jitter(Duration::from_secs(10))
                .grace(Duration::from_secs(5)),
        );
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .connection_lifetime_limits(limits.clone());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .connection_lifetime_limits(limits.clone())
            .with_graceful_shutdown(std::future::pending());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .with_graceful_shutdown(std::future::pending())
            .connection_lifetime_limits(limits.clone());
        serve(TcpListener::bind(addr).await.unwrap(), router.clone())
            .connection_lifetime_limits(limits.clone())
            .with_executor(TestExecutor::new());
    }

    async fn handler() {}

    #[derive(Clone)]
    struct TestExecutor(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl TestExecutor {
        fn new() -> Self {
            Self(std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)))
        }
    }

    impl super::Executor for TestExecutor {
        fn execute<Fut>(&self, fut: Fut) -> JoinHandle<Fut::Output>
        where
            Fut: std::future::Future + Send + 'static,
            Fut::Output: Send + 'static,
        {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tokio::spawn(fut)
        }
    }

    // Runtime check that a `Router` is served end-to-end through the re-exported
    // `axum::serve` over a real TCP connection. This exercises `Router`'s
    // `Service<IncomingStream>` make-service path, which the `TestClient`-based
    // tests bypass by wrapping services in `tower::make::Shared`.
    #[crate::test]
    async fn serve_router_over_tcp() {
        let app = Router::new().route("/", get(|| async { "Hello, World!" }));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(serve(listener, app).into_future());

        let stream = TokioIo::new(TcpStream::connect(addr).await.unwrap());
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().body(Body::empty()).unwrap();
        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(Body::new(response.into_body()), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"Hello, World!");
    }

    // `serve(listener, router)` used to run `self.clone().with_state(())` on
    // every accepted connection. The `Arc` behind `Router` is shared with
    // `serve`, so that fell back to deep-cloning every route's service for each
    // connection and kept the copy alive for as long as the connection was open.
    #[crate::test]
    async fn new_connection_does_not_clone_route_services() {
        use std::{
            convert::Infallible,
            future::{ready, Ready},
            sync::{
                atomic::{AtomicUsize, Ordering},
                Arc,
            },
            task::{Context, Poll},
        };

        use axum_core::response::IntoResponse;
        use tower_service::Service;

        struct Svc(Arc<AtomicUsize>);

        impl Clone for Svc {
            fn clone(&self) -> Self {
                self.0.fetch_add(1, Ordering::SeqCst);
                Self(self.0.clone())
            }
        }

        impl Service<Request> for Svc {
            type Response = crate::response::Response;
            type Error = Infallible;
            type Future = Ready<Result<Self::Response, Infallible>>;

            fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }

            fn call(&mut self, _req: Request) -> Self::Future {
                ready(Ok("ok".into_response()))
            }
        }

        async fn connect(
            addr: std::net::SocketAddr,
        ) -> hyper::client::conn::http1::SendRequest<Body> {
            let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            let (sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
                .await
                .unwrap();
            tokio::spawn(conn);
            sender
        }

        async fn clones_during(
            sender: &mut hyper::client::conn::http1::SendRequest<Body>,
            clones: &AtomicUsize,
        ) -> usize {
            let before = clones.load(Ordering::SeqCst);
            let request = Request::builder().uri("/").body(Body::empty()).unwrap();
            let response = sender.send_request(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            clones.load(Ordering::SeqCst) - before
        }

        let clones = Arc::new(AtomicUsize::new(0));
        let app = Router::new().route_service("/", Svc(clones.clone()));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(serve(listener, app).into_future());

        let mut first = connect(addr).await;
        clones_during(&mut first, &clones).await;
        let per_request = clones_during(&mut first, &clones).await;

        let mut second = connect(addr).await;
        let first_request_on_new_connection = clones_during(&mut second, &clones).await;

        assert_eq!(first_request_on_new_connection, per_request);
    }

    #[crate::test]
    async fn serving_with_connect_info_over_tcp() {
        use crate::extract::ConnectInfo;

        async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
            addr.to_string()
        }

        let app = Router::new().route("/", get(handler));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server = serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        );
        let server_addr = server.local_addr().unwrap();
        tokio::spawn(server.into_future());

        let client = TcpStream::connect(server_addr).await.unwrap();
        let client_addr = client.local_addr().unwrap();

        let stream = TokioIo::new(client);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(conn);

        let request = Request::builder().body(Body::empty()).unwrap();
        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(Body::new(response.into_body()), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body.parse::<SocketAddr>().unwrap(), client_addr);
    }
}
