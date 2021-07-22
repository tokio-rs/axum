//! Simple in-memory key/value store showing features of axum.
//!
//! Run with:
//!
//! ```not_rust
//! RUST_LOG=tower_http=debug,key_value_store=trace cargo run --example key_value_store
//! ```

use axum::{
    async_trait,
    extract::{extractor_middleware, ContentLengthLimit, Extension, RequestParts, UrlParams},
    prelude::*,
    response::IntoResponse,
    routing::BoxRoute,
    service::ServiceExt,
};
use bytes::Bytes;
use http::StatusCode;
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
    add_extension::AddExtensionLayer, compression::CompressionLayer, trace::TraceLayer,
};

#[tokio::main]
async fn main() {
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
    hyper::Server::bind(&addr)
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
    UrlParams((key,)): UrlParams<(String,)>,
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
    UrlParams((key,)): UrlParams<(String,)>,
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

fn admin_routes() -> BoxRoute<hyper::Body> {
    async fn delete_all_keys(Extension(state): Extension<SharedState>) {
        state.write().unwrap().db.clear();
    }

    async fn remove_key(
        UrlParams((key,)): UrlParams<(String,)>,
        Extension(state): Extension<SharedState>,
    ) {
        state.write().unwrap().db.remove(&key);
    }

    route("/keys", delete(delete_all_keys))
        .route("/key/:key", delete(remove_key))
        // Require beare auth for all admin routes
        .layer(extractor_middleware::<RequireAuth>())
        .boxed()
}

/// An extractor that performs authorization.
// TODO: when https://github.com/hyperium/http-body/pull/46 is merged we can use
// `tower_http::auth::RequireAuthorization` instead
struct RequireAuth;

#[async_trait]
impl<B> extract::FromRequest<B> for RequireAuth
where
    B: Send,
{
    type Rejection = StatusCode;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let auth_header = req
            .headers()
            .and_then(|headers| headers.get(http::header::AUTHORIZATION))
            .and_then(|value| value.to_str().ok());

        if let Some(value) = auth_header {
            if let Some(token) = value.strip_prefix("Bearer ") {
                if token == "secret-token" {
                    return Ok(Self);
                }
            }
        }

        Err(StatusCode::UNAUTHORIZED)
    }
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
