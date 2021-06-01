use http::{Request, StatusCode};
use hyper::Server;
use std::net::SocketAddr;
use tower::make::Shared;
use tower_web::{body::Body, extract, response::Html};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = tower_web::app()
        .at("/")
        .get(handler)
        .at("/greet/:name")
        .get(greet)
        // convert it into a `Service`
        .into_service();

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    let server = Server::bind(&addr).serve(Shared::new(app));
    server.await.unwrap();
}

async fn handler(_req: Request<Body>) -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

async fn greet(_req: Request<Body>, params: extract::UrlParamsMap) -> Result<String, StatusCode> {
    if let Some(name) = params.get("name") {
        Ok(format!("Hello {}!", name))
    } else {
        // if the route matches "name" will be present
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
