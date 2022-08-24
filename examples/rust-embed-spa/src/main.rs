use axum::{
    body::{boxed, Full},
    handler::Handler,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::Router,
};
use mime_guess;
use rust_embed::RustEmbed;
use std::net::SocketAddr;

static INDEX_HTML: &str = "index.html";

#[derive(RustEmbed)]
#[folder = "assets/"]
struct Assets;

#[tokio::main]
async fn main() {
    let app = Router::new().fallback(static_handler.into_service());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();

    if path.is_empty() || path == INDEX_HTML {
        return index_html().await.into_response();
    }

    match Assets::get(path.as_str()) {
        Some(content) => {
            let body = boxed(Full::from(content.data));
            let mime = mime_guess::from_path(path).first_or_octet_stream();

            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(body)
                .unwrap()
        }
        None => {
            if path.contains('.') {
                return not_found().await.into_response();
            }

            index_html().await.into_response()
        }
    }
}

async fn index_html() -> impl IntoResponse {
    match Assets::get(INDEX_HTML) {
        Some(content) => {
            let body = boxed(Full::from(content.data));

            Response::builder()
                .header(header::CONTENT_TYPE, "text/html")
                .body(body)
                .unwrap()
        }
        None => not_found().await.into_response(),
    }
}

async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "404")
}
