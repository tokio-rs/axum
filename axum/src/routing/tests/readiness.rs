use super::*;
use std::task::{Context, Poll};
use tower::Layer;

/// Panics if `call` is invoked on an instance that wasn't polled to readiness.
///
/// Each clone starts unready, and the first `poll_ready` on an instance returns
/// `Pending`, so callers must also handle backpressure.
struct ReadyCheck<S> {
    inner: S,
    polled_once: bool,
    ready: bool,
}

impl<S> ReadyCheck<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            polled_once: false,
            ready: false,
        }
    }
}

impl<S: Clone> Clone for ReadyCheck<S> {
    fn clone(&self) -> Self {
        Self::new(self.inner.clone())
    }
}

impl<S, R> Service<R> for ReadyCheck<S>
where
    S: Service<R>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if !self.polled_once {
            self.polled_once = true;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        let res = std::task::ready!(self.inner.poll_ready(cx));
        self.ready = res.is_ok();
        Poll::Ready(res)
    }

    fn call(&mut self, req: R) -> Self::Future {
        assert!(self.ready, "called without being polled to readiness");
        self.ready = false;
        self.inner.call(req)
    }
}

#[derive(Clone)]
struct ReadyCheckLayer;

impl<S> Layer<S> for ReadyCheckLayer {
    type Service = ReadyCheck<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ReadyCheck::new(inner)
    }
}

#[derive(Clone)]
struct Ok200;

impl Service<Request> for Ok200 {
    type Response = &'static str;
    type Error = Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Request) -> Self::Future {
        std::future::ready(Ok("ok"))
    }
}

fn leaf() -> ReadyCheck<Ok200> {
    ReadyCheck::new(Ok200)
}

async fn send_twice(app: Router, uri: &str) {
    // Same `Router` value twice, plus a fresh clone, so both the shared and the
    // cloned paths are exercised
    for app in [app.clone(), app.clone(), app] {
        let res = app
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{uri}");
    }
}

#[crate::test]
async fn services_are_polled_to_readiness_before_being_called() {
    let layered = |app: Router| {
        app.layer(ReadyCheckLayer)
            .layer(ReadyCheckLayer)
            .route_layer(ReadyCheckLayer)
    };

    let app = Router::new()
        .route_service("/service", leaf())
        .route("/get-service", get_service(leaf()))
        .route("/handler", get(|| async { "ok" }))
        .route(
            "/handler-layered",
            get(|| async { "ok" }).layer(ReadyCheckLayer),
        )
        .nest_service("/nested", leaf())
        .merge(Router::new().route_service("/merged", leaf()));
    let app = layered(app).fallback_service(leaf());

    for uri in [
        "/service",
        "/get-service",
        "/handler",
        "/handler-layered",
        "/nested/x",
        "/merged",
        "/fallback",
    ] {
        send_twice(app.clone(), uri).await;
        send_twice(app.clone().with_state(()), uri).await;
    }
}
