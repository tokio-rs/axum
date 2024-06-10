use axum::extract::Extension;
use axum_macros::debug_handler;

struct NonCloneType;

#[debug_handler]
async fn test_extension_non_clone(_: Extension<NonCloneType>) {}

fn main() {}
