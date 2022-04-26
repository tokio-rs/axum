mod multiplex_service;

use std::net::SocketAddr;
use tonic::transport::Server;

use rpc_helloworld::{greeter_server::Greeter, HelloReply, HelloRequest};
use tonic::{Response as TonicResponse, Status};

use axum::{response::Response, routing::get};
use tower::ServiceExt;
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

    // build our application with a route
    let web = axum::Router::new()
        .route("/", get(web_root).post(web_root))
        .map_err(|i| match i {});

    let hellowrld_service =
        rpc_helloworld::greeter_server::GreeterServer::new(GrpcServiceImpl::default());

    let grpc = Server::builder()
        .add_service(hellowrld_service)
        .into_service()
        .map_response(|response| {
            let (parts, body) = response.into_parts();
            Response::from_parts(parts, axum::body::boxed(body))
        });

    let service = multiplex_service::MultiplexService { web, grpc };

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    hyper::Server::bind(&addr)
        .tcp_keepalive(Some(std::time::Duration::from_secs(360)))
        .serve(service.make_shared())
        .await
        .unwrap();
}
