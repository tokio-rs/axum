Nest a [`Service`] at some path.

`nest_service` behaves in the same way as `nest` in terms of

- [How the URI changes](#how-the-uri-changes)
- [Captures from outer routes](#captures-from-outer-routes)
- [Differences to wildcard routes](#differences-to-wildcard-routes)

But differs with regards to [fallbacks]. See ["Differences between `nest` and
`nest_service`"](#differences-between-nest-and-nest_service) for more details.

# Example

`nest_service` can for example be used with [`tower_http::services::ServeDir`]
to serve static files from a directory:

```rust
use axum::{
    Router,
    routing::get_service,
    http::StatusCode,
    error_handling::HandleErrorLayer,
};
use std::{io, convert::Infallible};
use tower_http::services::ServeDir;

// Serves files inside the `public` directory at `GET /assets/*`
let serve_dir_service = get_service(ServeDir::new("public"))
    .handle_error(|error: io::Error| async move {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unhandled internal error: {}", error),
        )
    });

let app = Router::new().nest_service("/assets", serve_dir_service);
# let _: Router = app;
```

[fallbacks]: Router::fallback
