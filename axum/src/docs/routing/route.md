Add another route to the router.

`path` is a string of path segments separated by `/`. Each segment
can either be static, contain a capture, or be a wildcard.

`method_router` is the [`MethodRouter`] that should receive the request if the
path matches `path`. Usually, `method_router` will be a handler wrapped in a method
router like [`get`]. See [`handler`](crate::handler) for more details on handlers.

# Static paths

Examples:

- `/`
- `/foo`
- `/users/123`

If the incoming request matches the path exactly the corresponding service will
be called.

# Captures

Paths can contain segments like `/{key}` which matches any single segment and
will store the value captured at `key`. The value captured can be zero-length
except for in the invalid path `//`.

Each segment may have only one capture, but it may have static prefixes and suffixes.

Examples:

- `/{key}`
- `/users/{id}`
- `/users/{id}/tweets`
- `/avatars/{id}.jpg`
- `/avatars/{id}.png`

Captures can be extracted using [`Path`](crate::extract::Path). See its
documentation for more details.

It is not possible to create segments that only match some types like numbers or
regular expression. You must handle that manually in your handlers.

[`MatchedPath`] can be used to extract the matched path rather than the actual path.

Captures must not be empty. For example `/a/` will not match `/a/{capture}` and
`/.png` will not match `/{image}.png`.

You may have either capture(s) with static prefixes, capture(s) with suffixes, or a single
capture with both prefix and suffix, but these kinds of captures may not be mixed. You may mix
these with static routes and a standalone capture though. If multiple patterns match, static
segment takes precedence, then the capture with longest static prefix or suffix.

Example valid mixed route sets:
- `/logo.png`, `/author.jpg`, `/{id}.png`, `/{id}.jpg`, `/{other_file}` (but you may not add `/old-{id}.png` or `/post-{id}`).
- `/logo.png`, `/avatar-{id}.jpg`, `/{other_file}` (but you may not add `/{id}.jpg`, `/avatar-{id}.png`).

This is done on each level of the path and if the path matches even if due to a wildcard, that path
will be chosen. For example if one makes a request to `/foobar/baz` the first route will be used by
axum because it has better match on the leftmost differing path segment and the whole path matches.

- `/foobar/{*wildcard}`
- `/foo{wildcard}/baz`

# Wildcards

Paths can end in `/{*key}` which matches all segments and will store the segments
captured at `key`.

Examples:

- `/{*key}`
- `/assets/{*path}`
- `/{id}/{repo}/{*tree}`

Note that `/{*key}` doesn't match empty segments. Thus:

- `/{*key}` doesn't match `/` but does match `/a`, `/a/`, etc.
- `/x/{*key}` doesn't match `/x` or `/x/` but does match `/x/a`, `/x/a/`, etc.

Wildcard captures can also be extracted using [`Path`](crate::extract::Path):

```rust
use axum::{
    Router,
    routing::get,
    extract::Path,
};

let app: Router = Router::new().route("/{*key}", get(handler));

async fn handler(Path(path): Path<String>) -> String {
    path
}
```

Note that the leading slash is not included, i.e. for the route `/foo/{*rest}` and
the path `/foo/bar/baz` the value of `rest` will be `bar/baz`.

The captured segments can also be extracted as a sequence:

```rust
use axum::{
    Router,
    routing::get,
    extract::Path,
};

let app: Router = Router::new().route("/files/{*path}", get(handler));

async fn handler(Path(segments): Path<Vec<String>>) -> String {
    segments.join(", ")
}
```
For the path `/files/foo/bar/baz`, `segments` will be `["foo", "bar", "baz"]`.

# Accepting multiple methods

To accept multiple methods for the same route you can add all handlers at the
same time:

```rust
use axum::{Router, routing::{get, delete}, extract::Path};

let app = Router::new().route(
    "/",
    get(get_root).post(post_root).delete(delete_root),
);

async fn get_root() {}

async fn post_root() {}

async fn delete_root() {}
# let _: Router = app;
```

Or you can add them one by one:

```rust
# use axum::Router;
# use axum::routing::{get, post, delete};
#
let app = Router::new()
    .route("/", get(get_root))
    .route("/", post(post_root))
    .route("/", delete(delete_root));
#
# let _: Router = app;
# async fn get_root() {}
# async fn post_root() {}
# async fn delete_root() {}
```

# More examples

```rust
use axum::{Router, routing::{get, delete}, extract::Path};

let app = Router::new()
    .route("/", get(root))
    .route("/users", get(list_users).post(create_user))
    .route("/users/{id}", get(show_user))
    .route("/api/{version}/users/{id}/action", delete(do_users_action))
    .route("/assets/{*path}", get(serve_asset))
    .route("/batch/{*ids}", get(batch_process));

async fn root() {}

async fn list_users() {}

async fn create_user() {}

async fn show_user(Path(id): Path<u64>) {}

async fn do_users_action(Path((version, id)): Path<(String, u64)>) {}

async fn serve_asset(Path(path): Path<String>) {}

async fn batch_process(Path(ids): Path<Vec<u64>>) {}
# let _: Router = app;
```

# Panics

Panics if the route overlaps with another route:

```rust,should_panic
use axum::{routing::get, Router};

let app = Router::new()
    .route("/", get(|| async {}))
    .route("/", get(|| async {}));
# let _: Router = app;
```

The static route `/foo` and the dynamic route `/{key}` are not considered to
overlap and `/foo` will take precedence.

Also panics if `path` is empty.
