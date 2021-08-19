//! Simple in-memory key/value store showing features of axum.
//!
//! Run with:
//!
//! ```not_rust
//! cargo run -p example-key-value-store
//! ```

use axum::{
    body::Bytes,
    extract::{ContentLengthLimit, Extension, Path},
    handler::{delete, get, Handler},
    http::StatusCode,
    response::IntoResponse,
    route,
    routing::{BoxRoute, Router},
};
use std::{
    borrow::Cow,
    collections::HashMap,
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::Duration,
};
use tower::{BoxError, ServiceBuilder};
use tower_http::{
    add_extension::AddExtensionLayer, auth::RequireAuthorizationLayer,
    compression::CompressionLayer, trace::TraceLayer,
};

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "example_key_value_store=debug,tower_http=debug")
    }
    tracing_subscriber::fmt::init();

    // Build our application by composing routes
    let app = route(
        "/:key",
        // Add compression to `kv_get`
        get(kv_get.layer(CompressionLayer::new()))
            // But don't compress `kv_set`
            .post(kv_set),
    )
    .route("/keys", get(list_keys))
    // Nest our admin routes under `/admin`
    .nest("/admin", admin_routes())
    // Add middleware to all routes
    .layer(
        ServiceBuilder::new()
            .load_shed()
            .concurrency_limit(1024)
            .timeout(Duration::from_secs(10))
            .layer(TraceLayer::new_for_http())
            .layer(AddExtensionLayer::new(SharedState::default()))
            .into_inner(),
    )
    // Handle errors from middleware
    .handle_error(handle_error)
    .check_infallible();

    // Run our app with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

type SharedState = Arc<RwLock<State>>;

#[derive(Default)]
struct State {
    db: HashMap<String, Bytes>,
}

async fn kv_get(
    Path(key): Path<String>,
    Extension(state): Extension<SharedState>,
) -> Result<Bytes, StatusCode> {
    let db = &state.read().unwrap().db;

    if let Some(value) = db.get(&key) {
        Ok(value.clone())
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn kv_set(
    Path(key): Path<String>,
    ContentLengthLimit(bytes): ContentLengthLimit<Bytes, { 1024 * 5_000 }>, // ~5mb
    Extension(state): Extension<SharedState>,
) {
    state.write().unwrap().db.insert(key, bytes);
}

async fn list_keys(Extension(state): Extension<SharedState>) -> String {
    let db = &state.read().unwrap().db;

    db.keys()
        .map(|key| key.to_string())
        .collect::<Vec<String>>()
        .join("\n")
}

fn admin_routes() -> Router<BoxRoute> {
    async fn delete_all_keys(Extension(state): Extension<SharedState>) {
        state.write().unwrap().db.clear();
    }

    async fn remove_key(Path(key): Path<String>, Extension(state): Extension<SharedState>) {
        state.write().unwrap().db.remove(&key);
    }

    route("/keys", delete(delete_all_keys))
        .route("/key/:key", delete(remove_key))
        // Require bearer auth for all admin routes
        .layer(RequireAuthorizationLayer::bearer("secret-token"))
        .boxed()
}

fn handle_error(error: BoxError) -> Result<impl IntoResponse, Infallible> {
    if error.is::<tower::timeout::error::Elapsed>() {
        return Ok((StatusCode::REQUEST_TIMEOUT, Cow::from("request timed out")));
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return Ok((
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, try again later"),
        ));
    }

    Ok((
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {}", error)),
    ))
}
