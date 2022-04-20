mod arg;
mod response;

use axum::handler::Handler;
use axum::http::Request;
use crate::{Either, openapi};

use futures_core::Future;

pub use arg::DescribeHandlerArg;
pub use response::DescribeResponse;

pub struct OperationParts {
    pub parameters: Vec<openapi::Parameter>,
    pub request_body: Option<openapi::RequestBody>,
    pub responses: openapi::Responses,
}

pub trait DescribeHandler<T> {
    fn describe() -> OperationParts;
}

#[derive(Clone)]
pub struct DocumentedHandler<H> {
    pub handler: H,
    pub operation_id: &'static str,
    pub tags: &'static [&'static str],
    pub summary: &'static str,
    pub description: &'static str,
}

impl<T, B, H: Handler<T, B>> Handler<T, B> for DocumentedHandler<H> {
    type Future = H::Future;

    fn call(self, req: Request<B>) -> Self::Future {
        self.handler.call(req)
    }
}

impl<H> DocumentedHandler<H> {
    pub fn to_operation<T>(&self) -> openapi::Operation where H: DescribeHandler<T> {
        let OperationParts { parameters, request_body, responses } = H::describe();

        openapi::Operation {
            tags: self.tags,
            summary: self.summary,
            description: self.description,
            operation_id: self.operation_id,
            parameters,
            request_body,
            responses
        }
    }
}

impl OperationParts {
    fn push_handler_arg<T: DescribeHandlerArg>(&mut self) {
        match T::describe() {
            Some(Either::Left(param)) => self.parameters.push(param),
            Some(Either::Right(body)) => {
                if let Some(body) = self.request_body.replace(body) {
                    panic!("handler has more than one request body: {:?}", body)
                }
            },
            None => (),
        }
    }
}

macro_rules! impl_describe_handler_fn {
    ($($ty:ident),*) => (
        impl<F, Fut, Res, $($ty,)*> DescribeHandler<($($ty,)*)> for F
        where
            F: FnOnce($($ty,)*) -> Fut,
            Fut: Future<Output = Res>,
            Res: DescribeResponse,
            $( $ty: DescribeHandlerArg,)*
        {
            fn describe() -> OperationParts {
                let mut parts = OperationParts {
                    parameters: vec![],
                    request_body: None,
                    responses: Res::describe(),
                };

                $(parts.push_handler_arg::<$ty>();)*

                parts
            }
        }
    )
}

all_the_tuples!(impl_describe_handler_fn);