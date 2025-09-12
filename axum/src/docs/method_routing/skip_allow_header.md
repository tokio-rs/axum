Skip `Allow` Header for not implemented method

```rust
use axum::routing::get;
use axum::Router;

let app = Router::new().route("/", get(|| async {}).skip_allow_header());

// Our app now accepts
// - GET /
# let _: Router = app;
```
