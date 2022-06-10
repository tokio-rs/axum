//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-rest-grpc-multiplex
//! ```

use self::multiplex_service::{GrpcErrorAsJson, MultiplexService};
use axum::{extract::Json, routing::get, Router};
use once_cell::sync::OnceCell;
use proto::{
    greeter_server::{Greeter, GreeterServer},
    HelloReply, HelloRequest,
};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tonic::{Response as TonicResponse, Status};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod multiplex_service;

mod proto {
    tonic::include_proto!("helloworld");
}

struct GrpcServiceImpl {
    static_name: &'static str
}

#[tonic::async_trait]
impl Greeter for GrpcServiceImpl {
    async fn say_hello(
        &self,
        request: tonic::Request<HelloRequest>,
    ) -> Result<TonicResponse<HelloReply>, Status> {
        tracing::info!("Got a request from {:?}", request.remote_addr());

        let reply = HelloReply {
            message: format!("Hello {}, my name is {}.", request.into_inner().name, self.static_name),
        };

        Ok(TonicResponse::new(reply))
    }
}

/// axum::Handler only takes one parameter (request),
/// but tokio impls take two parameters (&self + request).
/// tokio provides &self from an Arc stored in the service,
/// so create GRPC_SERVICE as a static, pass it to tokio's GreeterServer::from_arc
///   and statically reference it in the json_wrap_grpc
static GRPC_SERVICE: OnceCell<Arc<GrpcServiceImpl>> = OnceCell::new();

/// given a gRPC RPC implementation function,
/// produce a closure that can be used as an axum Handler that:
/// 1. deserializes JSON into the request type
/// 2. calls the gRPC RPC implementation
/// 3. serializes the response back to JSON
fn json_wrap_grpc<'a, 'r, F, ReqT, ResT> (grpc_impl_func: F)
    -> impl FnOnce(Json<ReqT>)
        -> Pin<Box<dyn Future<Output = Result<Json<ResT>, GrpcErrorAsJson>> + Send + 'a>> + Clone + Send + Sized + 'static
where
    F: FnOnce(&'r GrpcServiceImpl, tonic::Request<ReqT>)
        -> Pin<Box<dyn Future<Output = Result<tonic::Response<ResT>, tonic::Status>> + Send + 'r>> + Clone + Send + Sync + 'static,
    for<'de> ReqT: serde::Deserialize<'de> + Send + 'a,
    ResT: serde::Serialize
{
    move |Json(req): Json<ReqT>| {
        Box::pin((|Json(req): Json<ReqT>| async move {
            let r = grpc_impl_func(GRPC_SERVICE.get().unwrap(), tonic::Request::new(req)).await;
            match r {
                Ok(r) => Ok(Json(r.into_inner())),
                Err(e) => Err(GrpcErrorAsJson(e))
            }
        })(Json(req)))
    }
}

async fn web_root() -> &'static str {
    "Hello, World!"
}

#[tokio::main]
async fn main() {
    match GRPC_SERVICE.set(Arc::new(GrpcServiceImpl {
        static_name: "HAL 9000"
    })) {
        Ok(_) => {}
        Err(_) => { panic!("GRPC_HANDLER created twice"); }
    }

    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "example_rest_grpc_multiplex=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // build the rest service
    let rest = Router::new()
    .route("/", get(web_root))
    .route("/Hello", axum::routing::any(json_wrap_grpc(GrpcServiceImpl::say_hello)));

    // build the grpc service
    let grpc = GreeterServer::from_arc(GRPC_SERVICE.get().unwrap().clone());

    // combine them into one service
    let service = MultiplexService::new(rest, grpc);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(tower::make::Shared::new(service))
        .await
        .unwrap();
}
