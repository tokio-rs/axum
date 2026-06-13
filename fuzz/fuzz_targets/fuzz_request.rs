#![no_main]
use libfuzzer_sys::fuzz_target;
use axum::{Router, routing::get, body::Body};
use http::{Request, Method, Uri};
use bytes::Bytes;

fuzz_target!(|data: &[u8]| {
    let _ = tokio::runtime::Runtime::new().map(|rt| {
        rt.block_on(async {
            let app = Router::new()
                .route("/", get(|| async { "ok" }))
                .route("/api/:id", get(|| async { "api" }));

            let uri = Uri::from_maybe_shared(Bytes::copy_from_slice(data))
                .unwrap_or(Uri::from_static("/"));
            
            let req = Request::builder()
                .method(Method::GET)
                .uri(uri)
                .body(Body::empty())
                .unwrap_or_else(|_| Request::new(Body::empty()));

            let _ = tower::ServiceExt::<Request<Body>>::ready(&mut app)
                .await
                .map(|svc| svc.call(req));
        })
    });
});
