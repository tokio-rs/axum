mod multiplex_service;

use std::net::SocketAddr;

use rpc_helloworld::{greeter_server::Greeter, HelloReply, HelloRequest};
use tonic::{Response as TonicResponse, Status};

use axum::routing::get;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod rpc_helloworld {
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
        println!("Got a request from {:?}", request.remote_addr());

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
    let rest = axum::Router::new().route("/", get(web_root).post(web_root));

    // build the grpc service
    let grpc = rpc_helloworld::greeter_server::GreeterServer::new(GrpcServiceImpl::default());

    let service = multiplex_service::MultiplexService { rest, grpc };

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    hyper::Server::bind(&addr)
        .tcp_keepalive(Some(std::time::Duration::from_secs(360)))
        .serve(tower::make::Shared::new(service))
        .await
        .unwrap();
}
