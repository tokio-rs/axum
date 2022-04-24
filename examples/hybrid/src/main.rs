#![feature(let_chains)]

mod grpc;
mod web;

mod hybrid;

use std::{convert::Infallible, net::SocketAddr};

use axum::response::Response;
use hyper::{server::conn::AddrStream, service::make_service_fn};
use tower::ServiceExt;
use tracing::info;
use tracing_subscriber::FmtSubscriber;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, BoxError>;

fn make_hybrid_service<Web, Grpc>(web: Web, grpc: Grpc) -> hybrid::HybridService<Web, Grpc> {
    hybrid::HybridService::new(web, grpc)
}

#[tokio::main]
async fn main() -> Result<()> {
    // initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    // build our application with a route
    let web = web::ApiService::build().map_err(|i| match i {});

    let grpc = grpc::GrpcService::build().map_response(|response| {
        let (parts, body) = response.into_parts();
        Response::from_parts(parts, axum::body::boxed(body))
    });

    let service = make_hybrid_service(web, grpc);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("listening on {}", addr);
    hyper::Server::bind(&addr)
        .tcp_keepalive(Some(std::time::Duration::from_secs(360)))
        .serve(make_service_fn(move |_conn: &AddrStream| {
            let hybrid_service = service.clone();
            async move { Ok::<_, Infallible>(hybrid_service) }
        }))
        .await?;

    Ok(())
}
