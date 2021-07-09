# axum

**WARNING:** axum is very much still work in progress. Nothing is released
to crates.io yet and you shouldn't be using this in production.

axum is a web application framework that focuses on ergonomics and modularity.

[![Build status](https://github.com/davidpdrsn/axum/workflows/CI/badge.svg)](https://github.com/davidpdrsn/axum/actions)
<!--
[![Crates.io](https://img.shields.io/crates/v/axum)](https://crates.io/crates/axum)
[![Documentation](https://docs.rs/axum/badge.svg)](https://docs.rs/axum)
[![Crates.io](https://img.shields.io/crates/l/axum)](LICENSE)
-->

More information about this crate can be found in the [crate documentation][docs].

## Goals

- Ease of use. Building web apps in Rust should be as easy as `async fn
handle(Request) -> Response`.
- Solid foundation. axum is built on top of tower and makes it easy to
plug in any middleware from the [tower] and [tower-http] ecosystem.
- Focus on routing, extracting data from requests, and generating responses.
Tower middleware can handle the rest.
- Macro free core. Macro frameworks have their place but axum focuses
on providing a core that is macro free.

## Usage example

```rust
use axum::prelude::*;
use hyper::Server;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = route("/", get(|| async { "Hello, World!" }));

    // run it with hyper on localhost:3000
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

See the [crate documentation][docs] for way more examples.

## Examples

The [examples] folder contains various examples of how to use axum. The
[docs] also have lots of examples

## Getting Help

In the axum's repo we also have a [number of examples][examples]
showing how to put everything together. You're also welcome to ask in the
[`#tower` Discord channel][chat] or open an [issue] with your question.

## Contributing

:balloon: Thanks for your help improving the project! We are so happy to have
you! We have a [contributing guide][guide] to help you get involved in the
axum project.

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in axum by you, shall be licensed as MIT, without any
additional terms or conditions.

[examples]: https://github.com/davidpdrsn/axum/tree/master/examples
[docs]: https://docs.rs/axum/0.1.0
[tower]: https://crates.io/crates/tower
[tower-http]: https://crates.io/crates/tower-http
[guide]: CONTRIBUTING.md
[chat]: https://discord.gg/tokio
[issue]: https://github.com/davidpdrsn/axum/issues/new
