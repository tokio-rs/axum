use std::future::Future;

use axum::routing::MethodNotAllowed;
use okapi::openapi3::{Components, Operation};

use crate::ToResponses;

pub trait ToOperation<T> {
    fn to_operation(&self, components: &mut Components) -> Operation;
}

impl<T> ToOperation<T> for MethodNotAllowed {
    fn to_operation(&self, components: &mut Components) -> Operation {
        unreachable!()
    }
}

impl<F, Fut, Res> ToOperation<()> for F
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Res> + Send,
    Res: ToResponses,
{
    fn to_operation(&self, components: &mut Components) -> Operation {
        let mut op = Operation::default();
        op.responses = Res::to_responses(components);
        op
    }
}

impl<F, Fut, Res, T1> ToOperation<(T1,)> for F
where
    F: FnOnce(T1) -> Fut,
    Fut: Future<Output = Res> + Send,
    Res: ToResponses,
{
    fn to_operation(&self, components: &mut Components) -> Operation {
        // TODO(david): next step: ToParameter
        todo!()

        // let mut op = Operation::default();
        // op.responses = Res::to_responses(components);
        // op
    }
}
