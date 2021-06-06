use http::{Request, StatusCode};
use hyper::Server;
use std::net::SocketAddr;
use tower::make::Shared;
use tower_web::prelude::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = route("/", get(handler)).route("/greet/:name", get(greet));

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    let server = Server::bind(&addr).serve(Shared::new(app));
    server.await.unwrap();
}

async fn handler(_req: Request<Body>) -> response::Html<&'static str> {
    response::Html("<h1>Hello, World!</h1>")
}

async fn greet(_req: Request<Body>, params: extract::UrlParamsMap) -> Result<String, StatusCode> {
    if let Some(name) = params.get("name") {
        Ok(format!("Hello {}!", name))
    } else {
        // if the route matches "name" will be present
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
