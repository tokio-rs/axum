use axum_macros::debug_handler;
use axum::extract::Extension;

struct NonCloneType;

#[debug_handler]
async fn test_extension_non_clone(_: Extension<NonCloneType>) {}

fn main() {}
