#![allow(warnings)]

use bytes::Bytes;
use http::{Request, StatusCode};
use hyper::Server;
use serde::Deserialize;
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
use tower_web::{body::Body, extract, response, Error};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = tower_web::app()
        .at("/:key")
        .get(get)
        .post(set)
        // convert it into a `Service`
        .into_service();

    // add some middleware
    let app = ServiceBuilder::new()
        .timeout(Duration::from_secs(10))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
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
    params: extract::UrlParamsMap,
    state: extract::Extension<SharedState>,
) -> Result<Bytes, Error> {
    let state = state.into_inner();
    let db = &state.lock().unwrap().db;

    let key = params.get("key")?;

    if let Some(value) = db.get(key) {
        Ok(value.clone())
    } else {
        Err(Error::Status(StatusCode::NOT_FOUND))
    }
}

async fn set(
    _req: Request<Body>,
    params: extract::UrlParamsMap,
    value: extract::BytesMaxLength<{ 1024 * 5_000 }>, // ~5mb
    state: extract::Extension<SharedState>,
) -> Result<response::Empty, Error> {
    let state = state.into_inner();
    let db = &mut state.lock().unwrap().db;

    let key = params.get("key")?;
    let value = value.into_inner();

    db.insert(key.to_string(), value);

    Ok(response::Empty)
}
