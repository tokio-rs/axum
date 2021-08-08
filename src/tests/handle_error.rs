use super::*;
use futures_util::future::{pending, ready};
use tower::{timeout::TimeoutLayer, MakeService};

async fn unit() {}

async fn forever() {
    pending().await
}

fn timeout() -> TimeoutLayer {
    TimeoutLayer::new(Duration::from_millis(10))
}

#[derive(Clone)]
struct Svc;

impl<R> Service<R> for Svc {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: R) -> Self::Future {
        ready(Ok(Response::new(Body::empty())))
    }
}

fn check_make_svc<M, R, T, E>(_make_svc: M)
where
    M: MakeService<(), R, Response = T, Error = E>,
{
}

fn handle_error<E>(_: E) -> Result<StatusCode, Infallible> {
    Ok(StatusCode::INTERNAL_SERVER_ERROR)
}

#[tokio::test]
async fn handler() {
    let app = route(
        "/",
        get(forever
            .layer(timeout())
            .handle_error(|_: BoxError| Ok::<_, Infallible>(StatusCode::REQUEST_TIMEOUT))),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);
}

#[tokio::test]
async fn handler_multiple_methods_first() {
    let app = route(
        "/",
        get(forever
            .layer(timeout())
            .handle_error(|_: BoxError| Ok::<_, Infallible>(StatusCode::REQUEST_TIMEOUT)))
        .post(unit),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);
}

#[tokio::test]
async fn handler_multiple_methods_middle() {
    let app = route(
        "/",
        delete(unit)
            .get(
                forever
                    .layer(timeout())
                    .handle_error(|_: BoxError| Ok::<_, Infallible>(StatusCode::REQUEST_TIMEOUT)),
            )
            .post(unit),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);
}

#[tokio::test]
async fn handler_multiple_methods_last() {
    let app = route(
        "/",
        delete(unit).get(
            forever
                .layer(timeout())
                .handle_error(|_: BoxError| Ok::<_, Infallible>(StatusCode::REQUEST_TIMEOUT)),
        ),
    );

    let addr = run_in_background(app).await;

    let client = reqwest::Client::new();

    let res = client
        .get(format!("http://{}/", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);
}

#[test]
fn service_propagates_errors() {
    let app = route::<_, Body>("/echo", service::post(Svc));

    check_make_svc::<_, _, _, hyper::Error>(app.into_make_service());
}

#[test]
fn service_nested_propagates_errors() {
    let app = route::<_, Body>("/echo", nest("/foo", service::post(Svc)));

    check_make_svc::<_, _, _, hyper::Error>(app.into_make_service());
}

#[test]
fn service_handle_on_method() {
    let app = route::<_, Body>(
        "/echo",
        service::get(Svc).handle_error(handle_error::<hyper::Error>),
    );

    check_make_svc::<_, _, _, Infallible>(app.into_make_service());
}

#[test]
fn service_handle_on_method_multiple() {
    let app = route::<_, Body>(
        "/echo",
        service::get(Svc)
            .post(Svc)
            .handle_error(handle_error::<hyper::Error>),
    );

    check_make_svc::<_, _, _, Infallible>(app.into_make_service());
}

#[test]
fn service_handle_on_router() {
    let app =
        route::<_, Body>("/echo", service::get(Svc)).handle_error(handle_error::<hyper::Error>);

    check_make_svc::<_, _, _, Infallible>(app.into_make_service());
}

#[test]
fn service_handle_on_router_still_impls_routing_dsl() {
    let app = route::<_, Body>("/echo", service::get(Svc))
        .handle_error(handle_error::<hyper::Error>)
        .route("/", get(unit));

    check_make_svc::<_, _, _, Infallible>(app.into_make_service());
}

#[test]
fn layered() {
    let app = route::<_, Body>("/echo", get(unit))
        .layer(timeout())
        .handle_error(handle_error::<BoxError>);

    check_make_svc::<_, _, _, Infallible>(app.into_make_service());
}

#[tokio::test] // async because of `.boxed()`
async fn layered_boxed() {
    let app = route::<_, Body>("/echo", get(unit))
        .layer(timeout())
        .boxed()
        .handle_error(handle_error::<BoxError>);

    check_make_svc::<_, _, _, Infallible>(app.into_make_service());
}
