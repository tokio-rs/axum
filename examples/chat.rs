use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::{sink::SinkExt, stream::StreamExt};

use tokio::sync::broadcast;

use axum::handler::get;
use axum::response::Html;
use axum::routing::RoutingDsl;
use axum::ws::{ws, Message, WebSocket};
use axum::{extract, route, AddExtensionLayer};

// Our shared state
struct AppState {
    user_map: Mutex<HashMap<String, ()>>,
    tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() {
    let user_map = Mutex::new(HashMap::new());
    let (tx, _rx) = broadcast::channel(100);

    let app_state = Arc::new(AppState { user_map, tx });

    let app = route("/", get(index))
        .route("/websocket", ws(websocket))
        .layer(AddExtensionLayer::new(app_state));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    hyper::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn websocket(stream: WebSocket, state: extract::Extension<Arc<AppState>>) {
    let state = state.0;

    // By splitting we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    // Empty string.
    let mut username = String::new();

    // Loop until a text message is found.
    while let Some(Ok(msg)) = receiver.next().await {
        if let Some(name) = msg.to_str() {
            // If username that is sent by client is not taken, fill username string.
            check_username(&state, &mut username, name);

            // If not empty we want to quit the loop else we want to quit function.
            if !username.is_empty() {
                break;
            } else {
                // Only send our client that username is taken.
                let _ = sender.send(Message::text("Username already taken.")).await;

                return;
            }
        }
    }

    // Subscribe before sending joined message.
    let mut rx = state.tx.subscribe();

    // Send joined message to all subscribers.
    let msg = format!("{} joined.", username);
    let _ = state.tx.send(msg);

    // This task will receive broadcast messages and send text message to our client.
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if let Err(_) = sender.send(Message::text(msg)).await {
                break;
            }
        }
    });

    // Clone things we want to pass to the receiving task.
    let tx = state.tx.clone();
    let name = username.clone();

    // This task will receive messages from client and send them to broadcast subscribers.
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Some(text) = msg.to_str() {
                // Add username before message.
                let _ = tx.send(format!("{}: {}", name, text));
            }
        }
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    // Send user left message.
    let msg = format!("{} left.", username);
    let _ = state.tx.send(msg);

    // Remove username from map so new clients can take it.
    state.user_map.lock().unwrap().remove(&username);
}

fn check_username(state: &AppState, string: &mut String, name: &str) {
    let mut user_map = state.user_map.lock().unwrap();

    if !user_map.contains_key(name) {
        user_map.insert(name.to_owned(), ());

        string.push_str(name);
    }
}

// Include utf-8 file at **compile** time.
async fn index() -> Html<&'static str> {
    Html(std::include_str!("chat/chat.html"))
}
