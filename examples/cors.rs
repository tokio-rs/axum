//! Run with
//!
//! ```not_rust
//! cargo run --example cors
//! ```

use axum::prelude::*;
use http::{HeaderValue, header};
use tower::ServiceBuilder;
use std::net::SocketAddr;
use tower_http::set_header::SetResponseHeaderLayer;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

	// define the CORS-headers you want to add as a Tower service
	let cors_middleware = ServiceBuilder::new()
	.layer(SetResponseHeaderLayer::<_, Body>::if_not_present(
		header::ACCESS_CONTROL_ALLOW_METHODS,
		HeaderValue::from_static("OPTION, GET, POST, PATCH, DELETE"),
	))
	.layer(SetResponseHeaderLayer::<_, Body>::if_not_present(
		header::ACCESS_CONTROL_ALLOW_ORIGIN,
		HeaderValue::from_static("*"),
	))
	.into_inner();

    // create application with a route and add the layer off middleware including a handler for OPTIONS requests.
    let app = route("/", get(handler).options(|| async { "" }))
	.layer(cors_middleware);

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> response::Html<&'static str> {
    response::Html("<h1>Hello, World!</h1>")
}
