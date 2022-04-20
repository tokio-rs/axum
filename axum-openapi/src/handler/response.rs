use crate::openapi;

pub trait DescribeResponse {
    fn describe() -> openapi::Responses;
}

impl DescribeResponse for () {
    fn describe() -> openapi::Responses {
        openapi::Responses {
            default: Some(openapi::Response {}),
            responses: Default::default(),
        }
    }
}