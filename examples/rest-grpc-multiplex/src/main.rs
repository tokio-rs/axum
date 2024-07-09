//! Run with
//!
//! ```not_rust
//! cargo run -p example-rest-grpc-multiplex
//! ```

use axum::{extract::Request, http::header::CONTENT_TYPE, routing::get, Router};
use proto::{
    greeter_server::{Greeter, GreeterServer},
    HelloReply, HelloRequest,
};
use std::net::SocketAddr;
use tonic::{Request as TonicRequest, Response as TonicResponse, Status};
use tower::{steer::Steer, make::Shared};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod proto {
    tonic::include_proto!("helloworld");

    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("helloworld_descriptor");
}

#[derive(Default)]
struct GrpcServiceImpl {}

#[tonic::async_trait]
impl Greeter for GrpcServiceImpl {
    async fn say_hello(
        &self,
        request: TonicRequest<HelloRequest>,
    ) -> Result<TonicResponse<HelloReply>, Status> {
        tracing::info!("Got a gRPC request from {:?}", request.remote_addr());

        let reply = HelloReply {
            message: format!("Hello {}!", request.into_inner().name),
        };

        Ok(TonicResponse::new(reply))
    }
}

async fn web_root() -> &'static str {
    tracing::info!("Got a REST request");

    "Hello, World!"
}

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_rest_grpc_multiplex=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // build the rest service
    let rest = Router::new().route("/", get(web_root));

    // build the grpc service
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    let grpc = tonic::transport::Server::builder()
        .add_service(reflection_service)
        .add_service(GreeterServer::new(GrpcServiceImpl::default()))
        .into_router();

    // combine them into one service
    let service = Steer::new(vec![rest, grpc], |req: &Request, _services: &[_]| {
        if req
            .headers()
            .get(CONTENT_TYPE)
            .map(|content_type| content_type.as_bytes())
            .filter(|content_type| content_type.starts_with(b"application/grpc"))
            .is_some()
        {
            1
        } else {
            0
        }
    });

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::debug!("listening on {}", addr);
    axum::serve(listener, Shared::new(service)).await.unwrap();
}
