//! Example websocket server.
//!
//! Run with
//!
//! ```
//! RUST_LOG=tower_http=debug,key_value_store=trace \
//!     cargo run \
//!     --features ws \
//!     --example websocket
//! ```

use http::HeaderMap;
use hyper::Server;
use std::net::SocketAddr;
use tower::make::Shared;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tower_web::{
    prelude::*,
    response::Html,
    ws::{ws, Message, WebSocket},
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = route("/ws", ws(handle_socket))
        // html and js to test that it all works
        .route(
            "/",
            get(|| async { Html("<script src='script.js'></script>") }),
        )
        .route(
            "/script.js",
            get(|| async {
                let mut headers = HeaderMap::new();
                headers.insert(
                    http::header::CONTENT_TYPE,
                    "text/javascript".parse().unwrap(),
                );

                let js = r#"
                    const socket = new WebSocket('ws://localhost:3000/ws');

                    socket.addEventListener('open', function (event) {
                        socket.send('Hello Server!');
                    });

                    socket.addEventListener('message', function (event) {
                        console.log('Message from server ', event.data);
                    });
                "#;

                (headers, js)
            }),
        )
        // logging so we can see whats going on
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    let server = Server::bind(&addr).serve(Shared::new(app));
    server.await.unwrap();
}

async fn handle_socket(mut socket: WebSocket) {
    if let Some(msg) = socket.recv().await {
        let msg = msg.unwrap();
        println!("Client says: {:?}", msg);
    }

    loop {
        socket.send(Message::text("Hi!")).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}
