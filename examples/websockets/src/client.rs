//! Based on tokio-tungstenite example websocket client, but with multiple
//! concurrent websocket clients in one package
//!
//! This example will connect to a server specified in the argument in the specified
//! number of threads, and then flood some test messages over websocket.
//! This will also print whatever it gets into stdout.
//!
//! Note that this is not currently optimized for performance, especially around
//! stdout mutex management. Rather it's intended to show an example of working with a
//! websocket server.
//!

//boilerplate
use clap::Parser;
use std::borrow::Cow;
use std::time::Instant;

//we need tungstenite for websocket impl (same library as what axum is using)
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
//stream splitting
use futures_util::{SinkExt, StreamExt};

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value_t = String::from("ws://127.0.0.1:3000/ws"))]
    url: String,
    #[arg(short, long, default_value_t = 2)]
    number_of_clients: usize,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let url = Url::parse(&args.url).expect("Invalid URL supplied!");
    let start_time = Instant::now();
    //spawn a whole bunch of clients
    let clients: Vec<JoinHandle<_>> = (0..args.number_of_clients)
        .into_iter()
        .map(|cli| {
            let uurl = url.clone();
            tokio::spawn(async move { spawn_client(uurl, cli).await })
        })
        .collect();

    //wait for our clients to exit
    futures::future::join_all(clients).await;
    let end_time = Instant::now();
    println!("Total time taken {:#?}.", end_time - start_time);
}

//creates a client connected to a given url. quietly exits on failure.
async fn spawn_client(url: Url, who: usize) {
    let ws_stream = match connect_async(url).await {
        Ok((stream, response)) => {
            println!("Handshake for client {} has been completed", who);
            println!("Server response was {:?}", response);
            stream
        }
        Err(e) => {
            println!("WebSocket handshake for client {who} failed with {e}!");
            return;
        }
    };

    let (mut sender, mut receiver) = ws_stream.split();

    //we can ping the server for start
    sender
        .send(Message::Ping("Hello, Server!".into()))
        .await
        .expect("Can not send!");

    //spawn an async sender to push some more messages into the server
    let mut send_task = tokio::spawn(async move {
        for i in 1..30 {
            // In any websocket error, break loop.
            if sender
                .send(Message::Text(format!("Message number {}...", i)))
                .await
                .is_err()
            {
                return;
            }

            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }

        // When we are done we may want our client to close connection...
        println!("Sending close to {}...", who);
        if let Err(e) = sender
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: Cow::from("Goodbye"),
            })))
            .await
        {
            println!("Could not send Close due to {:?}, probably it is ok?", e);
        };
    });

    //receiver just prints whatever it gets
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

    //wait for someone to finish and kill the other task
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
        },
        _ = (&mut recv_task) =>{
            send_task.abort();
        }
    }
}

//Familiar function to handle messages we get (with a slight twist that Frame variant is visible)
fn process_message(msg: Message, who: usize) -> bool {
    match msg {
        Message::Text(t) => {
            println!(">>> {} got str: {:?}", who, t);
        }
        Message::Binary(d) => {
            println!(">>> {} got {} bytes: {:?}", who, d.len(), d);
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>> {} got close with code {} and reason `{}`",
                    who, cf.code, cf.reason
                );
            } else {
                println!(">>> {} somehow got close message without CloseFrame", who);
            }
            return true;
        }

        Message::Pong(v) => {
            println!(">>> {} got pong with {:?}", who, v);
        }
        // You should never need to manually handle these, as tungstenite websocket library
        // will do so for you automagically by replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            println!(">>> {} got ping with {:?}", who, v);
        }

        Message::Frame(_) => {
            unreachable!("This is never supposed to happen")
        }
    }
    return false;
}
