use axum::{body::BoxBody, http::header::CONTENT_TYPE, response::IntoResponse};
use futures::{future::BoxFuture, ready};
use hyper::{Body, Request, Response};
use std::{
    convert::Infallible,
    task::{Context, Poll},
};
use tower::Service;

pub struct MultiplexService<A, B> {
    rest: A,
    rest_ready: bool,
    grpc: B,
    grpc_ready: bool,
}

impl<A, B> MultiplexService<A, B> {
    pub fn new(rest: A, grpc: B) -> Self {
        Self {
            rest,
            rest_ready: false,
            grpc,
            grpc_ready: false,
        }
    }
}

impl<A, B> Clone for MultiplexService<A, B>
where
    A: Clone,
    B: Clone,
{
    fn clone(&self) -> Self {
        Self {
            rest: self.rest.clone(),
            grpc: self.grpc.clone(),
            // the cloned services probably wont be ready
            rest_ready: false,
            grpc_ready: false,
        }
    }
}

impl<A, B> Service<Request<Body>> for MultiplexService<A, B>
where
    A: Service<Request<Body>, Error = Infallible>,
    A::Response: IntoResponse,
    A::Future: Send + 'static,
    B: Service<Request<Body>, Error = Infallible>,
    B::Response: IntoResponse,
    B::Future: Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // drive readiness for each inner service and record which is ready
        loop {
            match (self.rest_ready, self.grpc_ready) {
                (true, true) => {
                    return Ok(()).into();
                }
                (false, _) => {
                    ready!(self.rest.poll_ready(cx))?;
                    self.rest_ready = true;
                }
                (_, false) => {
                    ready!(self.grpc.poll_ready(cx))?;
                    self.grpc_ready = true;
                }
            }
        }
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // require users to call `poll_ready` first, if they don't we're allowed to panic
        // as per the `tower::Service` contract
        assert!(
            self.grpc_ready,
            "grpc service not ready. Did you forget to call `poll_ready`?"
        );
        assert!(
            self.rest_ready,
            "rest service not ready. Did you forget to call `poll_ready`?"
        );

        // if we get a grpc request call the grpc service, otherwise call the rest service
        // when calling a service it becomes not-ready so we have drive readiness again
        if is_grpc_request(&req) {
            self.grpc_ready = false;
            let future = self.grpc.call(req);
            Box::pin(async move {
                let res = future.await?;
                Ok(res.into_response())
            })
        } else {
            self.rest_ready = false;
            let future = self.rest.call(req);
            Box::pin(async move {
                let res = future.await?;
                Ok(res.into_response())
            })
        }
    }
}

fn is_grpc_request<B>(req: &Request<B>) -> bool {
    req.headers()
        .get(CONTENT_TYPE)
        .map(|content_type| content_type.as_bytes())
        .filter(|content_type| content_type.starts_with(b"application/grpc"))
        .is_some()
}

// wrapper type to convert GRPC errors to HTTP responses with JSON body
pub struct GrpcErrorAsJson(pub tonic::Status);

#[derive(serde::Serialize)]
struct GrpcStatus<'a> {
    grpc_error_code: i32,
    grpc_error_description: &'a str,
    message: &'a str
}

impl axum::response::IntoResponse for GrpcErrorAsJson {
    fn into_response(self) -> axum::response::Response {
        let json_status = GrpcStatus {
            grpc_error_code: self.0.code() as i32,
            grpc_error_description: self.0.code().description(),
            message: self.0.message()
        };
        let response_body = serde_json::to_string(&json_status).unwrap();

        // https://chromium.googlesource.com/external/github.com/grpc/grpc/+/refs/tags/v1.21.4-pre1/doc/statuscodes.md
        let code = match self.0.code() {
            tonic::Code::Ok => hyper::StatusCode::OK,
            tonic::Code::Cancelled => hyper::StatusCode::from_u16(499u16).unwrap(),
            tonic::Code::Unknown => hyper::StatusCode::INTERNAL_SERVER_ERROR,
            tonic::Code::InvalidArgument => hyper::StatusCode::BAD_REQUEST,
            tonic::Code::DeadlineExceeded => hyper::StatusCode::GATEWAY_TIMEOUT,
            tonic::Code::NotFound => hyper::StatusCode::NOT_FOUND,
            tonic::Code::AlreadyExists => hyper::StatusCode::CONFLICT,
            tonic::Code::PermissionDenied => hyper::StatusCode::FORBIDDEN,
            tonic::Code::ResourceExhausted => hyper::StatusCode::TOO_MANY_REQUESTS,
            tonic::Code::FailedPrecondition => hyper::StatusCode::BAD_REQUEST,
            tonic::Code::Aborted => hyper::StatusCode::CONFLICT,
            tonic::Code::OutOfRange => hyper::StatusCode::BAD_REQUEST,
            tonic::Code::Unimplemented => hyper::StatusCode::NOT_IMPLEMENTED,
            tonic::Code::Internal => hyper::StatusCode::INTERNAL_SERVER_ERROR,
            tonic::Code::Unavailable => hyper::StatusCode::SERVICE_UNAVAILABLE,
            tonic::Code::DataLoss => hyper::StatusCode::INTERNAL_SERVER_ERROR,
            tonic::Code::Unauthenticated => hyper::StatusCode::UNAUTHORIZED
        };

        let mut response = (code, response_body).into_response();
        response.headers_mut().insert(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("application/json"),
        );
        response
    }
}
