# Sharing state with handlers

It is common to share some state between handlers for example to share a
pool of database connections or clients to other services. That can be done
using the [`AddExtension`] middleware (applied with [`AddExtensionLayer`])
and the [`extract::Extension`] extractor:

```rust,no_run
use axum::{
    AddExtensionLayer,
    extract,
    routing::get,
    Router,
};
use std::sync::Arc;

struct State {
    // ...
}

let shared_state = Arc::new(State { /* ... */ });

let app = Router::new()
    .route("/", get(handler))
    .layer(AddExtensionLayer::new(shared_state));

async fn handler(
    state: extract::Extension<Arc<State>>,
) {
    let state: Arc<State> = state.0;

    // ...
}
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```
