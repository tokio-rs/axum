use axum::{
    debug_middleware,
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::{from_fn, Next},
    response::{ForceStatusCode, Response},
    Router,
};

#[debug_middleware]
async fn with_body(req: Request, next: Next) -> (ForceStatusCode, Response) {
    (ForceStatusCode(StatusCode::CREATED), next.run(req).await)
}

#[debug_middleware]
async fn with_headers(
    req: Request,
    next: Next,
) -> (axum::response::ForceStatusCode, HeaderMap, Response) {
    (
        ForceStatusCode(StatusCode::ACCEPTED),
        HeaderMap::new(),
        next.run(req).await,
    )
}

fn main() {
    let _: Router = Router::new().layer(from_fn(with_body));
    let _: Router = Router::new().layer(from_fn(with_headers));
}
