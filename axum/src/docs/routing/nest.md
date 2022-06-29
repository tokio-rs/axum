Nest a router at some path.

This allows you to break your application into smaller pieces and compose
them together.

# Example

```rust
use axum::{
    routing::{get, post},
    Router,
};

let user_routes = Router::new().route("/:id", get(|| async {}));

let team_routes = Router::new().route("/", post(|| async {}));

let api_routes = Router::new()
    .nest("/users", user_routes)
    .nest("/teams", team_routes);

let app = Router::new().nest("/api", api_routes);

// Our app now accepts
// - GET /api/users/:id
// - POST /api/teams
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

# How the URI changes

Note that nested routes will not see the original request URI but instead
have the matched prefix stripped. This is necessary for services like static
file serving to work. Use [`OriginalUri`] if you need the original request
URI.

# Captures from outer routes

Take care when using `nest` together with dynamic routes as nesting also
captures from the outer routes:

```rust
use axum::{
    extract::Path,
    routing::get,
    Router,
};
use std::collections::HashMap;

async fn users_get(Path(params): Path<HashMap<String, String>>) {
    // Both `version` and `id` were captured even though `users_api` only
    // explicitly captures `id`.
    let version = params.get("version");
    let id = params.get("id");
}

let users_api = Router::new().route("/users/:id", get(users_get));

let app = Router::new().nest("/:version/api", users_api);
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

# Differences to wildcard routes

Nested routes are similar to wildcard routes. The difference is that
wildcard routes still see the whole URI whereas nested routes will have
the prefix stripped:

```rust
use axum::{routing::get, http::Uri, Router};

let nested_router = Router::new()
    .route("/", get(|uri: Uri| async {
        // `uri` will _not_ contain `/bar`
    }));

let app = Router::new()
    .route("/foo/*rest", get(|uri: Uri| async {
        // `uri` will contain `/foo`
    }))
    .nest("/bar", nested_router);
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

# Differences between `nest` and `nest_service`

When [fallbacks] are called differs between `nest` and `nested_service`. Routers
nested with `nest` will delegate to the fallback if they don't have a matching
route, whereas `nested_service` will not.

```rust
use axum::{Router, routing::{get, any}, handler::Handler};

let nested_router = Router::new().route("/users", get(|| async {}));

let nested_service = Router::new().route("/app.js", get(|| async {}));

async fn fallback() {}

let app = Router::new()
    .nest("/api", nested_router)
    .nest_service("/assets", nested_service)
    // the fallback is not called for request starting with `/assets` but will be
    // called for requests starting with `/api` if `nested_router` doesn't have
    // a matching route
    .fallback(fallback.into_service());
# let _: Router = app;
```

Note that you would normally use [`tower_http::services::ServeDir`] for serving
static files and thus not call `nest_service` with a `Router`.

# Panics

- If the route overlaps with another route. See [`Router::route`]
for more details.
- If the route contains a wildcard (`*`).
- If `path` is empty.
- If the nested router has a [fallback](Router::fallback). This is because
  `Router` only allows a single fallback.

[`OriginalUri`]: crate::extract::OriginalUri
[fallbacks]: Router::fallback
