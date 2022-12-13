//! Example websocket server.
//!
//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-websockets
//! firefox http://localhost:3000
//! ```

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        TypedHeader,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, get_service},
    Router,
};

use std::borrow::Cow;
use std::{net::SocketAddr, path::PathBuf};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

//allows to extract the IP of connecting user
use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::CloseFrame;

//allows to split the websocket stream into separate TX and RX branches
use futures::{sink::SinkExt, stream::StreamExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "example_websockets=debug,tower_http=debug,tungstentite-rs=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    // build our application with some routes
    let app = Router::new()
        .fallback_service(
            get_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
                .handle_error(|error: std::io::Error| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {}", error),
                    )
                }),
        )
        // routes are matched from bottom to top, so we have to put `nest` at the
        // top since it matches all routes
        .route("/ws", get(ws_handler))
        // logging so we can see whats going on
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

/// A formally required handler for the request.
/// At this point we can extract useful TCP/IP metadata such as IP address of the client
/// as well as user-agent of the browser
async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{}` at {} connected.", user_agent, addr.to_string());
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

fn process_message(msg: Message, who: SocketAddr) -> bool {
    match msg {
        Message::Text(t) => {
            println!(">>> {} sent str: {:?}", who, t);
        }
        Message::Binary(d) => {
            println!(">>> {} sent {} bytes: {:?}", who, d.len(), d);
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>> {} sent close with code {} and reason `{}`",
                    who, cf.code, cf.reason
                );
            } else {
                println!(">>> {} somehow sent close message without CloseFrame", who);
            }
            return true;
        }

        Message::Pong(v) => {
            println!(">>> {} sent pong with {:?}", who, v);
        }
        // You should never need to manually handle these, as tungstentite websocket library
        // will do so for you automagically by replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            println!(">>> {} sent ping with {:?}", who, v);
        }
    }
    return false;
}

async fn handle_socket(mut socket: WebSocket, who: SocketAddr) {
    //send a ping (unsupported by some browsers) just to kick things off and get a response
    if let Ok(_) = socket.send(Message::Ping(vec![1, 2, 3])).await {
        println!("Pinged {}...", who);
    } else {
        println!("Could not ping {}!", who);
        return;
    }

    // receive single message form a client (we can either receive or send with socket)
    // this will likely be the Pong for our Ping or a hello.
    if let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            if process_message(msg, who) {
                return;
            }
        } else {
            println!("{} abruptly disconnected", who);
            return;
        }
    }

    // we can also send messages to client by simply sleeping,
    // since each client gets individual statemachine. Thus, waiting for this
    // client to finish getting his greetings does not prevent other clients form
    // connecting to server.
    for i in 1..5 {
        if socket
            .send(Message::Text(String::from(format!("Hi {} times!", i))))
            .await
            .is_err()
        {
            println!("client abruptly disconnected");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // By splitting socket we can send and receive at the same time. In this example we will send
    // unsolicited messages to client based on some sort of server's internal event (i.e .timer).
    let (mut sender, mut receiver) = socket.split();

    // Spawn a task that will push 100 messages to the client (does not matter what client does)
    let mut send_task = tokio::spawn(async move {
        let mut i = 20;
        while i > 0 {
            // In any websocket error, break loop.
            if sender
                .send(Message::Text(format!("{} messages left...", i)))
                .await
                .is_err()
            {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            i -= 1;
        }
        if i == 0 {
            println!("Sending close to {}...", who);
            if let Err(e) = sender
                .send(Message::Close(Some(CloseFrame {
                    code: 1000,
                    reason: Cow::from("Goodbye"),
                })))
                .await
            {
                println!("Could not send Close due to {}, probably it is ok?", e);
            }
        }
        i
    });

    // This second task will receive messages from client and print them on server console
    let mut recv_task = tokio::spawn(async move {
        let mut cnt = 0;
        while let Some(Ok(msg)) = receiver.next().await {
            cnt += 1;
            // print message and break if instructed to do so
            if process_message(msg, who) {
                break;
            }
        }
        cnt
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        rv_a = (&mut send_task) => {
            match rv_a {
                Ok(a)=>println!("{} messages remaining to send", a),
                Err(a)=>println!("Error sending messages {:?}", a)
            }
            recv_task.abort();
        },
        rv_b = (&mut recv_task) =>{
            match rv_b {
                Ok(b)=>println!("Received {} messages", b),
                Err(b)=>println!("Error receiving messages {:?}", b)
            }
            send_task.abort();
        }
    }

    // returning from the handler destroys the websocket context
    println!("Websocket context {} destroyed", who);
}
