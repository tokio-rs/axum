//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-rest-grpc-multiplex
//! ```

use self::multiplex_service::MultiplexService;
use axum::{routing::get, Router};
use proto::{
    greeter_server::{Greeter, GreeterServer},
    HelloReply, HelloRequest,
};
use std::net::SocketAddr;
use tonic::{Response as TonicResponse, Status};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod multiplex_service;

mod proto {
    tonic::include_proto!("helloworld");
}

#[derive(Default)]
struct GrpcServiceImpl {}

#[tonic::async_trait]
impl Greeter for GrpcServiceImpl {
    async fn say_hello(
        &self,
        request: tonic::Request<HelloRequest>,
    ) -> Result<TonicResponse<HelloReply>, Status> {
        tracing::info!("Got a request from {:?}", request.remote_addr());

        let reply = HelloReply {
            message: format!("Hello {}!", request.into_inner().name),
        };

        Ok(TonicResponse::new(reply))
    }
}

async fn web_root() -> &'static str {
    "Hello, World!"
}

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "example_rest_grpc_multiplex=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // build the rest service
    let rest = Router::new().route("/", get(web_root));

    // build the grpc service
    let grpc = GreeterServer::new(GrpcServiceImpl::default());

    // combine them into one service
    let service = MultiplexService::new(rest, grpc);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(tower::make::Shared::new(service))
        .await
        .unwrap();
}
