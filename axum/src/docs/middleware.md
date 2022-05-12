axum is unique in that it doesn't have its own bespoke middleware system and
instead integrates with [`tower`]. This means the ecosystem of [`tower`] and
[`tower-http`] middleware all work with axum.

While its not necessary to fully understand tower to write or use middleware
with axum, having at least a basic understanding of tower's concepts is
recommended. See [tower's guides][tower-guides] for a general introduction.
Reading the documentation for [`tower::ServiceBuilder`] is also recommended.

# Applying middleware

axum allows you to add middleware just about anywhere

- To entire routers with [`Router::layer`] and [`Router::route_layer`].
- To method routers with [`MethodRouter::layer`] and [`MethodRouter::route_layer`].
- To individual handlers with [`Handler::layer`].

## Applying multiple middleware

Its recommended to use [`tower::ServiceBuilder`] to apply multiple middleware at
once, instead of calling `layer` (or `route_layer`) repeatedly:

```rust
use axum::{
    routing::get,
    Extension,
    Router,
};
use tower_http::{trace::TraceLayer};
use tower::ServiceBuilder;

async fn handler() {}

#[derive(Clone)]
struct State {}

let app = Router::new()
    .route("/", get(handler))
    .layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(Extension(State {}))
    );
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

# Commonly used middleware

Some commonly used middleware are:

- [`TraceLayer`](tower_http::trace) for high level tracing/logging.
- [`CorsLayer`](tower_http::cors) for handling CORS.
- [`CompressionLayer`](tower_http::compression) for automatic compression of
  responses.
- [`RequestIdLayer`](tower_http::request_id) and
  [`PropagateRequestIdLayer`](tower_http::request_id) set and propagate request
  ids.
- [`TimeoutLayer`](tower::timeout::TimeoutLayer) for timeouts. Note this
  requires using [`HandleErrorLayer`](crate::error_handling::HandleErrorLayer)
  to convert timeouts to responses.

# Ordering

When you add middleware with [`Router::layer`] (or similar) all previously added
routes will be wrapped in the middleware. Generally speaking, this results in
middleware being executed from bottom to top.

So if you do this:

```rust
use axum::{routing::get, Router};

async fn handler() {}

# let layer_one = axum::Extension(());
# let layer_two = axum::Extension(());
# let layer_three = axum::Extension(());
#
let app = Router::new()
    .route("/", get(handler))
    .layer(layer_one)
    .layer(layer_two)
    .layer(layer_three);
# let app: Router<axum::body::Body> = app;
```

Think of the middleware as being layered like an onion where each new layer
wraps all previous layers:

```not_rust
        requests
           |
           v
+----- layer_three -----+
| +---- layer_two ----+ |
| | +-- layer_one --+ | |
| | |               | | |
| | |    handler    | | |
| | |               | | |
| | +-- layer_one --+ | |
| +---- layer_two ----+ |
+----- layer_three -----+
           |
           v
        responses
```

That is:

- First `layer_three` receives the request
- It then does its thing and passes the request onto `layer_two`
- Which passes the request onto `layer_one`
- Which passes the request onto `handler` where a response is produced
- That response is then passes to `layer_one`
- Then to `layer_two`
- And finally to `layer_three` where it's returned out of your app

It's a little more complicated in practice because any middleware is free to
return early and not call the next layer, for example if a request cannot be
authorized, but its a useful mental model to have.

As previously mentioned its recommended to add multiple middleware using
`tower::ServiceBuilder`, however this impacts ordering:

```rust
use tower::ServiceBuilder;
use axum::{routing::get, Router};

async fn handler() {}

# let layer_one = axum::Extension(());
# let layer_two = axum::Extension(());
# let layer_three = axum::Extension(());
#
let app = Router::new()
    .route("/", get(handler))
    .layer(
        ServiceBuilder::new()
            .layer(layer_one)
            .layer(layer_two)
            .layer(layer_three),
    );
# let app: Router<axum::body::Body> = app;
```

`ServiceBuilder` works by composing all layers into one such that they run top
to bottom. So with the previous code `layer_one` would receive the request
first, then `layer_two`, then `layer_three`, then `handler`, and then the
response would bubble back up through `layer_three`, then `layer_two`, and
finally `layer_one`.

Executing middleware top to bottom is generally easier to understand and follow
mentally which is one of the reasons `ServiceBuilder` is recommended.

# Writing middleware

axum offers many ways of writing middleware, at different levels of abstraction
and with different pros and cons.

## `axum::middleware::from_fn`

Use [`axum::middleware::from_fn`] to write your middleware when:

- You're not comfortable with implementing your own futures and would rather use
  the familiar `async`/`await` syntax.
- You don't intend to publish your middleware as a crate for others to use.
  Middleware written like this are only compatible with axum.

## `axum::middleware::from_extractor`

Use [`axum::middleware::from_extractor`] to write your middleware when:

- You have a type that you sometimes want to use as an extractor and sometimes
  as a middleware. If you only need your type as a middleware prefer
  [`middleware::from_fn`].

## tower's combinators

tower has several utility combinators that can be used to perform simple
modifications to requests or responses. The most commonly used ones are

- [`ServiceBuilder::map_request`]
- [`ServiceBuilder::map_response`]
- [`ServiceBuilder::then`]
- [`ServiceBuilder::and_then`]

You should use these when

- You want to perform a small ad hoc operation, such as adding a header.
- You don't intend to publish your middleware as a crate for others to use.

## `tower::Service` and `Pin<Box<dyn Future>>`

For maximum control (and a more low level API) you can write you own middleware
by implementing [`tower::Service`]:

Use [`tower::Service`] with `Pin<Box<dyn Future>>` to write your middleware when:

- Your middleware needs to be configurable for example via builder methods on
  your [`tower::Layer`] such as [`tower_http::trace::TraceLayer`].
- You do intend to publish your middleware as a crate for others to use.
- You're not comfortable with implementing your own futures.

A decent template for such a middleware could be:

```rust
use axum::{
    response::Response,
    body::Body,
    http::Request,
};
use futures::future::BoxFuture;
use tower::{Service, Layer};
use std::task::{Context, Poll};

struct MyLayer;

impl<S> Layer<S> for MyLayer {
    type Service = MyMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MyMiddleware { inner }
    }
}

#[derive(Clone)]
struct MyMiddleware<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for MyMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    // `BoxFuture` is a type alias for `Pin<Box<dyn Future + Send + 'a>>`
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let future = self.inner.call(request);
        Box::pin(async move {
            let response: Response = future.await?;
            Ok(response)
        })
    }
}
```

## `tower::Service` and custom futures

If you're comfortable implementing your own futures (or want to learn it) and
need as much control as possible then using `tower::Service` without boxed
futures is the way to go.

Use [`tower::Service`] with manual futures to write your middleware when:

- You want your middleware to have the lowest possible overhead.
- Your middleware needs to be configurable for example via builder methods on
  your [`tower::Layer`] such as [`tower_http::trace::TraceLayer`].
- You do intend to publish your middleware as a crate for others to use, perhaps
  as part of tower-http.
- You're comfortable with implementing your own futures, or want to learn how
  the lower levels of async Rust works.

tower's ["Building a middleware from scratch"][tower-from-scratch-guide]
guide is a good place to learn how to do this.

# Error handling for middleware

axum's error handling model requires handlers to always return a response.
However middleware is one possible way to introduce errors into an application.
If hyper receives an error the connection will be closed without sending a
response. Thus axum requires those errors to be handled gracefully:

```rust
use axum::{
    routing::get,
    error_handling::HandleErrorLayer,
    http::StatusCode,
    BoxError,
    Router,
};
use tower::{ServiceBuilder, timeout::TimeoutLayer};
use std::time::Duration;

async fn handler() {}

let app = Router::new()
    .route("/", get(handler))
    .layer(
        ServiceBuilder::new()
            // this middleware goes above `TimeoutLayer` because it will receive
            // errors returned by `TimeoutLayer`
            .layer(HandleErrorLayer::new(|_: BoxError| async {
                StatusCode::REQUEST_TIMEOUT
            }))
            .layer(TimeoutLayer::new(Duration::from_secs(10)))
    );
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

See [`error_handling`](crate::error_handling) for more details on axum's error
handling model.

# Routing to services/middleware and backpressure

Generally routing to one of multiple services and backpressure doesn't mix
well. Ideally you would want ensure a service is ready to receive a request
before calling it. However, in order to know which service to call, you need
the request...

One approach is to not consider the router service itself ready until all
destination services are ready. That is the approach used by
[`tower::steer::Steer`].

Another approach is to always consider all services ready (always return
`Poll::Ready(Ok(()))`) from `Service::poll_ready` and then actually drive
readiness inside the response future returned by `Service::call`. This works
well when your services don't care about backpressure and are always ready
anyway.

axum expects that all services used in your app wont care about
backpressure and so it uses the latter strategy. However that means you
should avoid routing to a service (or using a middleware) that _does_ care
about backpressure. At the very least you should [load shed] so requests are
dropped quickly and don't keep piling up.

It also means that if `poll_ready` returns an error then that error will be
returned in the response future from `call` and _not_ from `poll_ready`. In
that case, the underlying service will _not_ be discarded and will continue
to be used for future requests. Services that expect to be discarded if
`poll_ready` fails should _not_ be used with axum.

One possible approach is to only apply backpressure sensitive middleware
around your entire app. This is possible because axum applications are
themselves services:

```rust
use axum::{
    routing::get,
    Router,
};
use tower::ServiceBuilder;
# let some_backpressure_sensitive_middleware =
#     tower::layer::util::Identity::new();

async fn handler() { /* ... */ }

let app = Router::new().route("/", get(handler));

let app = ServiceBuilder::new()
    .layer(some_backpressure_sensitive_middleware)
    .service(app);
# async {
# axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
# };
```

However when applying middleware around your whole application in this way
you have to take care that errors are still being handled with
appropriately.

Also note that handlers created from async functions don't care about
backpressure and are always ready. So if you're not using any Tower
middleware you don't have to worry about any of this.

# Sharing state between handlers and middleware

State can be shared between middleware and handlers using [request extensions]:

```rust
use axum::{
    Router,
    http::{Request, StatusCode},
    routing::get,
    response::IntoResponse,
    middleware::{self, Next},
    extract::Extension,
};

#[derive(Clone)]
struct CurrentUser { /* ... */ }

async fn auth<B>(mut req: Request<B>, next: Next<B>) -> impl IntoResponse {
    let auth_header = req.headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    let auth_header = if let Some(auth_header) = auth_header {
        auth_header
    } else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if let Some(current_user) = authorize_current_user(auth_header).await {
        req.extensions_mut().insert(current_user);
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn authorize_current_user(auth_token: &str) -> Option<CurrentUser> {
    // ...
    # unimplemented!()
}

async fn handler(
    // extract the current user, set by the middleware
    Extension(current_user): Extension<CurrentUser>,
) {
    // ...
}

let app = Router::new()
    .route("/", get(handler))
    .route_layer(middleware::from_fn(auth));
# let app: Router = app;
```

[Response extensions] can also be used but note that request extensions are not
automatically moved to response extensions. You need to manually do that for the
extensions you need.

[`tower`]: https://crates.io/crates/tower
[`tower-http`]: https://crates.io/crates/tower-http
[tower-guides]: https://github.com/tower-rs/tower/tree/master/guides
[`axum::middleware::from_fn`]: crate::middleware::from_fn
[`middleware::from_fn`]: crate::middleware::from_fn
[tower-from-scratch-guide]: https://github.com/tower-rs/tower/blob/master/guides/building-a-middleware-from-scratch.md
[`ServiceBuilder::map_request`]: tower::ServiceBuilder::map_request
[`ServiceBuilder::map_response`]: tower::ServiceBuilder::map_response
[`ServiceBuilder::then`]: tower::ServiceBuilder::then
[`ServiceBuilder::and_then`]: tower::ServiceBuilder::and_then
[`axum::middleware::from_extractor`]: crate::middleware::from_extractor
[`Handler::layer`]: crate::handler::Handler::layer
[`Router::layer`]: crate::routing::Router::layer
[`MethodRouter::layer`]: crate::routing::MethodRouter::layer
[`Router::route_layer`]: crate::routing::Router::route_layer
[`MethodRouter::route_layer`]: crate::routing::MethodRouter::route_layer
[request extensions]: https://docs.rs/http/latest/http/request/struct.Request.html#method.extensions
[Response extensions]: https://docs.rs/http/latest/http/response/struct.Response.html#method.extensions
