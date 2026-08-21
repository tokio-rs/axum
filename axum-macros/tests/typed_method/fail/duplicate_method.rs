use axum::routing::MethodFilter;
use axum_macros::TypedMethod;

#[derive(TypedMethod)]
#[typed_method(MethodFilter::GET)]
#[typed_method(MethodFilter::POST)]
struct MyPath;

fn main() {}
