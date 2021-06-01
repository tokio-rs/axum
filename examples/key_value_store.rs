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
use tower_web::{body::Body, extract, handler::Handler};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = tower_web::app()
        .at("/:key")
        .get(get.layer(CompressionLayer::new()))
        .post(set)
        // convert it into a `Service`
        .into_service();

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

async fn get(
    _req: Request<Body>,
    params: extract::UrlParams<(String,)>,
    state: extract::Extension<SharedState>,
) -> Result<Bytes, StatusCode> {
    let state = state.into_inner();
    let db = &state.lock().unwrap().db;

    let key = params.into_inner();

    if let Some(value) = db.get(&key) {
        Ok(value.clone())
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn set(
    _req: Request<Body>,
    params: extract::UrlParams<(String,)>,
    value: extract::BytesMaxLength<{ 1024 * 5_000 }>, // ~5mb
    state: extract::Extension<SharedState>,
) {
    let state = state.into_inner();
    let db = &mut state.lock().unwrap().db;

    let key = params.into_inner();
    let value = value.into_inner();

    db.insert(key, value);
}
