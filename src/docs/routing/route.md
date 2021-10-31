Add another route to the router.

`path` is a string of path segments separated by `/`. Each segment
can be either static, a capture, or a wildcard.

`service` is the [`Service`] that should receive the request if the path
matches `path`. `service` will commonly be a handler. See
[`handler`](crate::handler) for more details on handlers.

# Static paths

Examples:

- `/`
- `/foo`
- `/users/123`

If the incoming request matches the path exactly the corresponding service will
be called.

# Captures

Paths can contain segments like `/:key` which matches any single segment and
will store the value captured at `key`.

Examples:

- `/:key`
- `/users/:id`
- `/users/:id/tweets`

Captures can be extracted using [`Path`](crate::extract::Path). See its
documentation for more details.

It is not possible to create segments that only match some types like numbers or
regular expression. You must handle that manually in your handlers.

[`MatchedPath`](crate::extract::MatchedPath) can be used to extract the matched
path rather than the actual path.

# Wildcards

Paths can end in `/*key` which matches all segments and will store the segments
captured at `key`.

Examples:

- `/*key`
- `/assets/*path`
- `/:id/:repo/*tree`

Wildcard captures can also be extracted using [`Path`](crate::extract::Path).

# More examples

```rust
use axum::{Router, routing::{get, delete}, extract::Path};

let app = Router::new()
    .route("/", get(root))
    .route("/users", get(list_users).post(create_user))
    .route("/users/:id", get(show_user))
    .route("/api/:version/users/:id/action", delete(do_users_action))
    .route("/assets/*path", get(serve_asset));

async fn root() {}

async fn list_users() {}

async fn create_user() {}

async fn show_user() {}

async fn do_users_action() {}

async fn serve_asset(Path(path): Path<String>) {}
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

# Panics

Panics if the route overlaps with another route:

```rust,should_panic
use axum::{routing::get, Router};

let app = Router::new()
    .route("/", get(|| async {}))
    .route("/", get(|| async {}));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

The static route `/foo` and the dynamic route `/:key` are not considered to
overlap and `/foo` will take precedence.

Take care when using [`Router::nest`] as it behaves like a wildcard route.
Therefore this setup panics:

```rust,should_panic
use axum::{routing::get, Router};

let app = Router::new()
    // this is similar to `/api/*`
    .nest("/api", get(|| async {}))
    // which overlaps with this route
    .route("/api/users", get(|| async {}));
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

Also panics if `path` is empty.

## Nesting

`route` cannot be used to nest `Router`s. Instead use [`Router::nest`].

Attempting to will result in a panic:

```rust,should_panic
use axum::{routing::get, Router};

let app = Router::new().route(
    "/",
    Router::new().route("/foo", get(|| async {})),
);
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```
