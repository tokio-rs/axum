use http::Request;
use hyper::Server;
use std::net::SocketAddr;
use tower::{make::Shared, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tower_web::{body::Body, response::Html};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = tower_web::app()
        .at("/")
        .get(handler)
        // convert it into a `Service`
        .into_service();

    // add some middleware
    let app = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .service(app);

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    let server = Server::bind(&addr).serve(Shared::new(app));
    server.await.unwrap();
}

async fn handler(_req: Request<Body>) -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}
