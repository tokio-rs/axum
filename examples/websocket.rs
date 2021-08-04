//! Example websocket server.
//!
//! Run with
//!
//! ```not_rust
//! RUST_LOG=tower_http=debug,key_value_store=trace cargo run --features=ws,headers --example websocket
//! ```

use axum::{
    extract::TypedHeader,
    prelude::*,
    routing::nest,
    service::ServiceExt,
    ws::{ws, Message, WebSocket},
};
use http::StatusCode;
use std::net::SocketAddr;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = nest(
        "/",
        axum::service::get(
            ServeDir::new("examples/websocket")
                .append_index_html_on_directories(true)
                .handle_error(|error: std::io::Error| {
                    Ok::<_, std::convert::Infallible>((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {}", error),
                    ))
                }),
        ),
    )
    // routes are matched from bottom to top, so we have to put `nest` at the
    // top since it matches all routes
    .route("/ws", ws(handle_socket))
    // logging so we can see whats going on
    .layer(
        TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::default().include_headers(true)),
    );

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handle_socket(
    mut socket: WebSocket,
    // websocket handlers can also use extractors
    user_agent: Option<TypedHeader<headers::UserAgent>>,
) {
    if let Some(TypedHeader(user_agent)) = user_agent {
        println!("`{}` connected", user_agent.as_str());
    }

    if let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            println!("Client says: {:?}", msg);
        } else {
            println!("client disconnected");
            return;
        }
    }

    loop {
        if socket.send(Message::text("Hi!")).await.is_err() {
            println!("client disconnected");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}
