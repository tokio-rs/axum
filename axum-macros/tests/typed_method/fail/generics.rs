use axum::routing::MethodFilter;
use axum_macros::TypedMethod;

#[derive(TypedMethod)]
#[typed_method(MethodFilter::GET)]
struct MyPath<T>(T);

fn main() {}
