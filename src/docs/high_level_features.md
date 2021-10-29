# High level features

- Route requests to handlers with a macro free API.
- Declaratively parse requests using extractors.
- Simple and predictable error handling model.
- Generate responses with minimal boilerplate.
- Take full advantage of the [`tower`] and [`tower-http`] ecosystem of
  middleware, services, and utilities.

In particular the last point is what sets `axum` apart from other frameworks.
`axum` doesn't have its own middleware system but instead uses
[`tower::Service`]. This means `axum` gets timeouts, tracing, compression,
authorization, and more, for free. It also enables you to share middleware with
applications written using [`hyper`] or [`tonic`].
