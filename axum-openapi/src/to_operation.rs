use okapi::openapi3::Operation;

pub trait ToOperation<T> {
    fn to_operation(&self) -> Operation;
}
