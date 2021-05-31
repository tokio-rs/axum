# tower-web

This is *not* https://github.com/carllerche/tower-web even though the name is
the same. Its just a prototype of a minimal HTTP framework I've been toying
with.

# What is this?

## Goals

- As easy to use as tide. I don't really consider warp easy to use due to type
  tricks it uses. `fn route() -> impl Filter<...>` also isn't very ergonomic.
  Just `async fn(Request) -> Result<Response, Error>` would be nicer.
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

Defining a single route looks like this:

```rust
let app = tower_web::app().at("/").get(root);

async fn root(req: Request<Body>) -> Result<&'static str, Error> {
    Ok("Hello, World!")
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
async fn handler(req: Request<Body>) -> Result<&'static str, Error> {
    Ok("Hello, World!")
}
```

They most take the request as the first argument but all arguments following
are called "extractors" and are used to extract data from the request (similar
to rocket but no macros):

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
) -> Result<&'static str, Error> {
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
) -> Result<&'static str, Error> {
    // ...
}
```

You can also get the raw response body:

```rust
async fn handler(
    req: Request<Body>,
    // buffer the whole request body
    body: Bytes,
) -> Result<&'static str, Error> {
    // ...
}
```

Or limit the body size:

```rust
async fn handler(
    req: Request<Body>,
    // max body size in bytes
    body: extract::BytesMaxLength<1024>,
) -> Result<&'static str, Error> {
    Ok("Hello, World!")
}
```

Anything that implements `FromRequest` can work as an extractor where
`FromRequest` is a simple async trait:

```rust
#[async_trait]
pub trait FromRequest: Sized {
    async fn from_request(req: &mut Request<Body>) -> Result<Self, Error>;
}
```

This "extractor" pattern is inspired by Bevy's ECS. The idea is that it should
be easy to parse pick apart the request without having to repeat yourself a lot
or use macros.

Dynamic routes like `GET /users/:id` is also supported.

You can also return different response types:

```rust
async fn string_response(req: Request<Body>) -> Result<String, Error> {
    // ...
}

// gets `content-type: appliation/json`. `Json` can contain any `T: Serialize`
async fn json_response(req: Request<Body>) -> Result<response::Json<User>, Error> {
    // ...
}

// gets `content-type: text/html`. `Html` can contain any `T: Into<Bytes>`
async fn html_response(req: Request<Body>) -> Result<response::Html<String>, Error> {
    // ...
}

// or for full control
async fn response(req: Request<Body>) -> Result<Response<Body>, Error> {
    // ...
}
```

You can also apply Tower middleware to single routes:

```rust
let app = tower_web::app()
    .at("/")
    .get(send_some_large_file.layer(tower_http::compression::CompressionLayer::new()))
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

- Error handling should probably be redone. Not quite sure if its bad the
  `Error` is just an enum where everything is public.
- Audit which error codes we return for each kind of error. This will probably
  be changed when error handling is re-done.
- Probably don't want to require `hyper::Body` for request bodies. Should
  have our own so hyper isn't required.
- `RouteBuilder` should have an `async fn serve(self) -> Result<(),
  hyper::Error>` for users who just wanna create a hyper server and not care
  about the lower level details. Should be gated by a `hyper` feature.
- Each new route makes a new allocation for the response body, since `Or` needs
  to unify the response body types. Would be nice to find a way to avoid that.
- It should be possible to package some routes together and apply a tower
  middleware to that collection and then merge those routes into the app.
