# tower-web

This is *not* https://github.com/carllerche/tower-web even though the name is
the same. Its just a prototype of a minimal HTTP framework I've been toying
with. Will probably change the name to something else.

# What is this?

## Goals

- As easy to use as tide. I don't really consider warp easy to use due to type
  tricks it uses. `fn route() -> impl Filter<...>` also isn't very ergonomic.
  Just `async fn(Request) -> Response` would be nicer.
- Deep integration with Tower meaning you can
    - Apply middleware to the entire application.
    - Apply middleware to a single route.
    - Apply middleware to subset of routes.
- Just focus on routing and generating responses. Tower can do the rest.
  Want timeouts? Use `tower::timeout::Timeout`. Want logging? Use
  `tower_http::trace::Trace`.
- Work with Tokio. tide is cool but requires async-std.
- Not macro based. Heavy macro based APIs can be very ergonomic but comes at a
  complexity cost. Would like to see if I can design an API that is ergonomic
  and doesn't require macros.

## Non-goals

- Runtime independent. If becoming runtime independent isn't too much then fine
  but explicitly designing for runtime independence isn't a goal.
- Speed. As long as things are reasonably fast that is fine. For example using
  async-trait for ergonomics is fine even though it comes at a cost.

# Example usage

NOTE: Error handling has changed quite a bit and these examples are slightly out
of date. See the examples for working examples.

Defining a single route looks like this:

```rust
let app = tower_web::app().at("/").get(root);

async fn root(req: Request<Body>) -> &'static str {
    "Hello, World!"
}
```

Adding more routes follows the same pattern:

```rust
let app = tower_web::app()
    .at("/")
    .get(root)
    .at("/users")
    .get(users_index)
    .post(users_create);
```

Handler functions are just async functions like:

```rust
async fn handler(req: Request<Body>) -> &'static str {
    "Hello, World!"
}
```

They must take the request as the first argument but all arguments following
are called "extractors" and are used to extract data from the request (similar
to rocket but without macros):

```rust
#[derive(Deserialize)]
struct UserPayload {
    username: String,
}

#[derive(Deserialize)]
struct Pagination {
    page: usize,
    per_page: usize,
}

async fn handler(
    req: Request<Body>,
    // deserialize response body with `serde_json` into a `UserPayload`
    user: extract::Json<UserPayload>,
    // deserialize query string into a `Pagination`
    pagination: extract::Query<Pagination>,
) -> &'static str {
    let user: UserPayload = user.into_inner();
    let pagination: Pagination = pagination.into_inner();

    // ...
}
```

The inputs can also be optional:

```rust
async fn handler(
    req: Request<Body>,
    user: Option<extract::Json<UserPayload>>,
) -> &'static str {
    // ...
}
```

You can also get the raw response body:

```rust
async fn handler(
    req: Request<Body>,
    // buffer the whole request body
    body: Bytes,
) -> &'static str {
    // ...
}
```

Or limit the body size:

```rust
async fn handler(
    req: Request<Body>,
    // max body size in bytes
    body: extract::BytesMaxLength<1024>,
) -> &'static str {
    // ...
}
```

Params from dynamic routes like `GET /users/:id` can be extracted like so

```rust
async fn handle(
    req: Request<Body>,
    // get a map of key value pairs
    map: extract::UrlParamsMap,
) -> &'static str {
    let raw_id: Option<&str> = map.get("id");
    let parsed_id: Option<i32> = map.get_typed::<i32>("id");

    // ...
}

async fn handle(
    req: Request<Body>,
    // or get a tuple with each param
    params: extract::UrlParams<(i32, String)>,
) -> &'static str {
    let (id, name) = params.into_inner();

    // ...
}
```

Anything that implements `FromRequest` can work as an extractor where
`FromRequest` is an async trait:

```rust
#[async_trait]
pub trait FromRequest: Sized {
    type Rejection: IntoResponse<B>;

    async fn from_request(req: &mut Request<Body>) -> Result<Self, Self::Rejection>;
}
```

This "extractor" pattern is inspired by Bevy's ECS. The idea is that it should
be easy to pick apart the request without having to repeat yourself a lot or use
macros.

The return type must implement `IntoResponse`:

```rust
async fn empty_response(req: Request<Body>) {
    // ...
}

// gets `content-type: text/plain`
async fn string_response(req: Request<Body>) -> String {
    // ...
}

// gets `content-type: appliation/json`. `Json` can contain any `T: Serialize`
async fn json_response(req: Request<Body>) -> response::Json<User> {
    // ...
}

// gets `content-type: text/html`. `Html` can contain any `T: Into<Bytes>`
async fn html_response(req: Request<Body>) -> response::Html<String> {
    // ...
}

// or for full control
async fn response(req: Request<Body>) -> Response<Body> {
    // ...
}

// Result is supported if each type implements `IntoResponse`
async fn response(req: Request<Body>) -> Result<Html<String>, StatusCode> {
    // ...
}
```

This makes error handling quite simple. Basically handlers are not allowed to
fail and must always produce a response. This also means users are in charge of
how their errors are mapped to responses rather than a framework providing some
opaque catch all error type.

You can also apply Tower middleware to single routes:

```rust
let app = tower_web::app()
    .at("/")
    .get(send_some_large_file.layer(CompressionLayer::new()))
```

Or to the whole app:

```rust
let service = tower_web::app()
    .at("/")
    .get(root)
    .into_service()

let app = ServiceBuilder::new()
    .timeout(Duration::from_secs(30))
    .layer(TraceLayer::new_for_http())
    .layer(CompressionLayer::new())
    .service(app);
```

And of course run it with Hyper:

```rust
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = tower_web::app()
        .at("/")
        .get(handler)
        // convert it into a `Service`
        .into_service();

    // add some middleware
    let app = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .service(app);

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    let server = Server::bind(&addr).serve(Shared::new(app));
    server.await.unwrap();
}
```

See the examples directory for more examples.

# TODO

- `RouteBuilder` should have an `async fn serve(self) -> Result<(),
  hyper::Error>` for users who just wanna create a hyper server and not care
  about the lower level details. Should be gated by a `hyper` feature.
- Each new route makes a new allocation for the response body, since `Or` needs
  to unify the response body types. Would be nice to find a way to avoid that.
- It should be possible to package some routes together and apply a tower
  middleware to that collection and then merge those routes into the app.
