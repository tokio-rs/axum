//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-static-file-server
//! ```

use axum::{
    body::Body,
    handler::HandlerWithoutStateExt,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{get, get_service},
    Router,
};
use axum_extra::routing::SpaRouter;
use std::{io, net::SocketAddr};
use tower::ServiceExt;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_static_file_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tokio::join!(
        serve(using_spa_router(), 3000),
        serve(using_serve_dir(), 3001),
        serve(using_serve_dir_with_assets_fallback(), 3002),
        serve(using_serve_dir_only_from_root_via_fallback(), 3003),
        serve(using_serve_dir_with_handler_as_service(), 3004),
        serve(two_serve_dirs(), 3005),
        serve(calling_serve_dir_from_a_handler(), 3006),
    );
}

fn using_spa_router() -> Router {
    // `SpaRouter` is the easiest way to serve assets at a nested route like `/assets`
    //
    // Requests starting with `/assets` will be served from files in the current directory.
    // Requests to unknown routes will get `index.html`.
    Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .merge(SpaRouter::new("/assets", "assets").index_file("index.html"))
}

fn using_serve_dir() -> Router {
    // `SpaRouter` is just a convenient wrapper around `ServeDir`
    //
    // You can use `ServeDir` directly to further customize your setup
    let serve_dir = get_service(ServeDir::new("assets")).handle_error(handle_error);

    Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .nest_service("/assets", serve_dir.clone())
        .fallback_service(serve_dir)
}

fn using_serve_dir_with_assets_fallback() -> Router {
    // for example `ServeDir` allows setting a fallback if an asset is not found
    // so with this `GET /assets/doesnt-exist.jpg` will return `index.html`
    // rather than a 404
    let serve_dir = ServeDir::new("assets").not_found_service(ServeFile::new("assets/index.html"));
    let serve_dir = get_service(serve_dir).handle_error(handle_error);

    Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .nest_service("/assets", serve_dir.clone())
        .fallback_service(serve_dir)
}

fn using_serve_dir_only_from_root_via_fallback() -> Router {
    // you can also serve the assets directly from the root (not nested under `/assets`)
    // by only setting a `ServeDir` as the fallback
    let serve_dir = ServeDir::new("assets").not_found_service(ServeFile::new("assets/index.html"));
    let serve_dir = get_service(serve_dir).handle_error(handle_error);

    Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .fallback_service(serve_dir)
}

fn using_serve_dir_with_handler_as_service() -> Router {
    async fn handle_404() -> (StatusCode, &'static str) {
        (StatusCode::NOT_FOUND, "Not found")
    }

    // you can convert handler function to service
    let service = handle_404
        .into_service()
        .map_err(|err| -> std::io::Error { match err {} });

    let serve_dir = ServeDir::new("assets").not_found_service(service);
    let serve_dir = get_service(serve_dir).handle_error(handle_error);

    Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .fallback_service(serve_dir)
}

fn two_serve_dirs() -> Router {
    // you can also have two `ServeDir`s nested at different paths
    let serve_dir_from_assets = get_service(ServeDir::new("assets")).handle_error(handle_error);
    let serve_dir_from_dist = get_service(ServeDir::new("dist")).handle_error(handle_error);

    Router::new()
        .nest_service("/assets", serve_dir_from_assets)
        .nest_service("/dist", serve_dir_from_dist)
}

#[allow(clippy::let_and_return)]
fn calling_serve_dir_from_a_handler() -> Router {
    // via `tower::Service::call`, or more conveniently `tower::ServiceExt::oneshot` you can
    // call `ServeDir` yourself from a handler
    Router::new().nest_service(
        "/foo",
        get(|request: Request<Body>| async {
            let service = get_service(ServeDir::new("assets")).handle_error(handle_error);
            let result = service.oneshot(request).await;
            result
        }),
    )
}

async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}

async fn serve(app: Router, port: u16) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.layer(TraceLayer::new_for_http()).into_make_service())
        .await
        .unwrap();
}
