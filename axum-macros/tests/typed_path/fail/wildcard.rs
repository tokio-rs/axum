use axum_extra::routing::TypedPath;

#[derive(TypedPath)]
#[typed_path("/users/*rest")]
struct MyPath;

fn main() {}
