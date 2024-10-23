use std::{
    collections::HashMap,
    convert::Infallible,
    marker::PhantomData,
    sync::Arc,
    task::{Context, Poll},
};

use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::MethodRouter,
};
use futures::{future::BoxFuture, FutureExt};
use tower::Service;

trait CommandFromBody {
    fn command_from_body(body: &[u8]) -> Option<&str>;
}

struct ExampleService<C> {
    routes: Arc<HashMap<String, MethodRouter>>,
    _phantom_c: PhantomData<fn() -> C>,
}

impl<C> Service<Request> for ExampleService<C>
where
    C: CommandFromBody,
{
    type Error = Infallible;
    type Response = Response;
    type Future = BoxFuture<'static, Result<Response, Infallible>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let routes = self.routes.clone();
        async move {
            let (parts, body) = req.into_parts();

            let Ok(bytes) = axum::body::to_bytes(body, usize::MAX).await else {
                return Ok(StatusCode::INTERNAL_SERVER_ERROR.into_response());
            };

            match C::command_from_body(&bytes).and_then(|cmd| routes.get(cmd)) {
                Some(router) => {
                    let req = Request::from_parts(parts, Body::from(bytes));

                    router.call_with_state(req, ()).await
                }

                None => Ok(StatusCode::NOT_FOUND.into_response()),
            }
        }
        .boxed()
    }
}
