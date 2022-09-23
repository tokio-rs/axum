Nest a [`Service`] at some path.

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

# Fallbacks

When nesting a router, if a request matches the prefix but the nested router doesn't have a matching
route, the outer fallback will _not_ be called:

```rust
use axum::{routing::get, http::StatusCode, handler::Handler, Router};

async fn fallback() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

let api_routes = Router::new().nest_service("/users", get(|| async {}));

let app = Router::new()
    .nest("/api", api_routes)
    .fallback(fallback);
# let _: Router = app;
```

Here requests like `GET /api/not-found` will go into `api_routes` and then to
the fallback of `api_routes` which will return an empty `404 Not Found`
response. The outer fallback declared on `app` will _not_ be called.

Think of nested services as swallowing requests that matches the prefix and
not falling back to outer router even if they don't have a matching route.

You can still add separate fallbacks to nested routers:

```rust
use axum::{routing::get, http::StatusCode, handler::Handler, Json, Router};
use serde_json::{json, Value};

async fn fallback() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

async fn api_fallback() -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_FOUND, Json(json!({ "error": "Not Found" })))
}

let api_routes = Router::new()
    .nest_service("/users", get(|| async {}))
    // add dedicated fallback for requests starting with `/api`
    .fallback(api_fallback);

let app = Router::new()
    .nest("/api", api_routes)
    .fallback(fallback);
# let _: Router = app;
```

# Panics

- If the route overlaps with another route. See [`Router::route`]
for more details.
- If the route contains a wildcard (`*`).
- If `path` is empty.

[`OriginalUri`]: crate::extract::OriginalUri
[fallbacks]: Router::fallback
