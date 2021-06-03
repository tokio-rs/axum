use bytes::Bytes;
use http::{Request, StatusCode};
use hyper::Server;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tower::{make::Shared, ServiceBuilder};
use tower_http::{
    add_extension::AddExtensionLayer, compression::CompressionLayer, trace::TraceLayer,
};
use tower_web::{
    body::Body,
    extract::{BytesMaxLength, Extension, UrlParams},
    get, route, Handler,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = route(
        "/:key",
        get(kv_get.layer(CompressionLayer::new())).post(kv_set),
    );

    // add some middleware
    let app = ServiceBuilder::new()
        .timeout(Duration::from_secs(10))
        .layer(TraceLayer::new_for_http())
        .layer(AddExtensionLayer::new(SharedState::default()))
        .service(app);

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    let server = Server::bind(&addr).serve(Shared::new(app));
    server.await.unwrap();
}

type SharedState = Arc<Mutex<State>>;

#[derive(Default)]
struct State {
    db: HashMap<String, Bytes>,
}

async fn kv_get(
    _req: Request<Body>,
    UrlParams((key,)): UrlParams<(String,)>,
    Extension(state): Extension<SharedState>,
) -> Result<Bytes, StatusCode> {
    let db = &state.lock().unwrap().db;

    if let Some(value) = db.get(&key) {
        Ok(value.clone())
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn kv_set(
    _req: Request<Body>,
    UrlParams((key,)): UrlParams<(String,)>,
    BytesMaxLength(value): BytesMaxLength<{ 1024 * 5_000 }>, // ~5mb
    Extension(state): Extension<SharedState>,
) {
    state.lock().unwrap().db.insert(key, value);
}
