use axum_macros::TypedPath;

#[derive(TypedPath)]
#[typed_path("/users")]
#[typed_method(axum::routing::MethodFilter::GET)]
struct Users;

fn main() {}
