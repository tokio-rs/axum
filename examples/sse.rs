use axum::{extract::TypedHeader, prelude::*, routing::nest, service::ServiceExt, sse::Event};
use futures::stream::{self, Stream};
use http::StatusCode;
use std::{convert::Infallible, net::SocketAddr, time::Duration};
use tokio_stream::StreamExt as _;
use tower_http::{services::ServeDir, trace::TraceLayer};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = nest(
        "/",
        axum::service::get(
            ServeDir::new("examples/sse")
                .append_index_html_on_directories(true)
                .handle_error(|error: std::io::Error| {
                    Ok::<_, std::convert::Infallible>((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled interal error: {}", error),
                    ))
                }),
        ),
    )
    .route("/sse", axum::sse::sse(make_stream))
    .layer(TraceLayer::new_for_http());

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    hyper::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn make_stream(
    // sse handlers can also use extractors
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
) -> Result<impl Stream<Item = Result<Event, Infallible>>, Infallible> {
    println!("`{}` connected", user_agent.as_str());

    // A `Stream` that repeats an event every second
    let stream = stream::repeat_with(|| Event::default().data("hi!"))
        .map(Ok)
        .throttle(Duration::from_secs(1));

    Ok(stream)
}
