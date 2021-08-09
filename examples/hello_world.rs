use axum::prelude::*;
use std::{convert::Infallible, net::SocketAddr, time::Duration};
use tower::{limit::RateLimitLayer, BoxError, ServiceBuilder};

#[tokio::main]
async fn main() {
    let handler_layer = ServiceBuilder::new()
        .buffer(1024)
        .layer(RateLimitLayer::new(10, Duration::from_secs(10)))
        .into_inner();

    let app = route(
        "/",
        get(handler
            .layer(handler_layer)
            .handle_error(|error: BoxError| {
                Ok::<_, Infallible>(format!("Unhandled error: {}", error))
            })),
    );

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> response::Html<&'static str> {
    response::Html("<h1>Hello, World!</h1>")
}
