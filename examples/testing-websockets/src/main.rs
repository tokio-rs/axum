//! Run with
//!
//! ```not_rust
//! cargo test -p example-testing-websockets
//! ```

use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
    response::Response,
    routing::get,
    Router,
};
use futures::{Sink, SinkExt, Stream, StreamExt};

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app()).await.unwrap();
}

fn app() -> Router {
    // WebSocket routes can generally be tested in two ways:
    //
    // - Integration tests where you run the server and connect with a real WebSocket client.
    // - Unit tests where you mock the socket as some generic send/receive type
    //
    // Which version you pick is up to you. Generally we recommend the integration test version
    // unless your app has a lot of setup that makes it hard to run in a test.
    Router::new()
        .route("/integration-testable", get(integration_testable_handler))
        .route("/unit-testable", get(unit_testable_handler))
}

// A WebSocket handler that echos any message it receives.
//
// This one we'll be integration testing so it can be written in the regular way.
async fn integration_testable_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(integration_testable_handle_socket)
}

async fn integration_testable_handle_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(msg) = msg {
            if socket
                .send(Message::Text(format!("You said: {msg}").into()))
                .await
                .is_err()
            {
                break;
            }
        }
    }
}

// The unit testable version requires some changes.
//
// By splitting the socket into an `impl Sink` and `impl Stream` we can test without providing a
// real socket and instead using channels, which also implement `Sink` and `Stream`.
async fn unit_testable_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(|socket| {
        let (write, read) = socket.split();
        unit_testable_handle_socket(write, read)
    })
}

// The implementation is largely the same as `integration_testable_handle_socket` expect we call
// methods from `SinkExt` and `StreamExt`.
async fn unit_testable_handle_socket<W, R>(mut write: W, mut read: R)
where
    W: Sink<Message> + Unpin,
    R: Stream<Item = Result<Message, axum::Error>> + Unpin,
{
    while let Some(Ok(msg)) = read.next().await {
        if let Message::Text(msg) = msg {
            if write
                .send(Message::Text(format!("You said: {msg}").into()))
                .await
                .is_err()
            {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::IntoFuture,
        net::{Ipv4Addr, SocketAddr},
    };
    use tokio_tungstenite::tungstenite;

    // We can integration test one handler by running the server in a background task and
    // connecting to it like any other client would.
    #[tokio::test]
    async fn integration_test() {
        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(axum::serve(listener, app()).into_future());

        let (mut socket, _response) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/integration-testable"))
                .await
                .unwrap();

        socket
            .send(tungstenite::Message::text("foo"))
            .await
            .unwrap();

        let msg = match socket.next().await.unwrap().unwrap() {
            tungstenite::Message::Text(msg) => msg,
            other => panic!("expected a text message but got {other:?}"),
        };

        assert_eq!(msg.as_str(), "You said: foo");
    }

    // We can unit test the other handler by creating channels to read and write from.
    #[tokio::test]
    async fn unit_test() {
        // Need to use "futures" channels rather than "tokio" channels as they implement `Sink` and
        // `Stream`
        let (socket_write, mut test_rx) = futures::channel::mpsc::channel(1024);
        let (mut test_tx, socket_read) = futures::channel::mpsc::channel(1024);

        tokio::spawn(unit_testable_handle_socket(socket_write, socket_read));

        test_tx.send(Ok(Message::Text("foo".into()))).await.unwrap();

        let msg = match test_rx.next().await.unwrap() {
            Message::Text(msg) => msg,
            other => panic!("expected a text message but got {other:?}"),
        };

        assert_eq!(msg.as_str(), "You said: foo");
    }
}
