//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-consume-body-in-extractor-or-middleware
//! ```

use axum::{
    async_trait,
    body::{self, BoxBody, Bytes, Full},
    extract::FromRequest,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_consume_body_in_extractor_or_middleware=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new().route("/", post(handler)).layer(
        ServiceBuilder::new()
            .map_request_body(body::boxed)
            .layer(middleware::from_fn(print_request_body)),
    );

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// middleware that shows how to consume the request body upfront
async fn print_request_body(
    request: Request<BoxBody>,
    next: Next<BoxBody>,
) -> Result<impl IntoResponse, Response> {
    let request = buffer_request_body(request).await?;

    Ok(next.run(request).await)
}

// the trick is to take the request apart, buffer the body, do what you need to do, then put
// the request back together
async fn buffer_request_body(request: Request<BoxBody>) -> Result<Request<BoxBody>, Response> {
    let (parts, body) = request.into_parts();

    // this wont work if the body is an long running stream
    let bytes = hyper::body::to_bytes(body)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response())?;

    do_thing_with_request_body(bytes.clone());

    Ok(Request::from_parts(parts, body::boxed(Full::from(bytes))))
}

fn do_thing_with_request_body(bytes: Bytes) {
    tracing::debug!(body = ?bytes);
}

async fn handler(BufferRequestBody(body): BufferRequestBody) {
    tracing::debug!(?body, "handler received body");
}

// extractor that shows how to consume the request body upfront
struct BufferRequestBody(Bytes);

// we must implement `FromRequest` (and not `FromRequestParts`) to consume the body
#[async_trait]
impl<S> FromRequest<S, BoxBody> for BufferRequestBody
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request<BoxBody>, state: &S) -> Result<Self, Self::Rejection> {
        let body = Bytes::from_request(req, state)
            .await
            .map_err(|err| err.into_response())?;

        do_thing_with_request_body(body.clone());

        Ok(Self(body))
    }
}
