# Community Projects

If your project isn't listed here and you would like it to be, please feel free to create a PR.

## Community maintained axum ecosystem

- [axum-server](https://crates.io/crates/axum-server): axum-server is a hyper server implementation designed to be used with axum.
- [axum-typed-websockets](https://crates.io/crates/axum-typed-websockets): `axum::extract::ws` with type safe messages.
- [tower-cookies](https://crates.io/crates/tower-cookies): Cookie manager middleware
- [axum-flash](https://crates.io/crates/axum-flash): One-time notifications (aka flash messages) for axum.
- [axum-msgpack](https://crates.io/crates/axum-msgpack): MessagePack Extractors for axum.
- [axum-sqlx-tx](https://crates.io/crates/axum-sqlx-tx): Request-bound [SQLx](https://github.com/launchbadge/sqlx#readme) transactions with automatic commit/rollback based on response.
- [aliri_axum](https://docs.rs/aliri_axum) and [aliri_tower](https://docs.rs/aliri_tower): JWT validation middleware and OAuth2 scopes enforcing extractors.
- [ezsockets](https://github.com/gbaranski/ezsockets): Easy to use WebSocket library that integrates with Axum.
- [axum_database_sessions](https://github.com/AscendingCreations/AxumSessions): Database persistent sessions like pythons flask_sessionstore for Axum.
- [axum_sessions_auth](https://github.com/AscendingCreations/AxumSessionsAuth): Persistant session based user login with rights management for Axum.
- [axum-auth](https://crates.io/crates/axum-auth): High-level http auth extractors for axum.
- [shuttle](https://github.com/getsynth/shuttle): A serverless platform built for Rust. Now with axum support.
- [axum-tungstenite](https://github.com/davidpdrsn/axum-tungstenite): WebSocket connections for axum directly using tungstenite
- [axum-jrpc](https://github.com/0xdeafbeef/axum-jrpc): Json-rpc extractor for axum
- [axum-tracing-opentelemetry](https://crates.io/crates/axum-tracing-opentelemetry): Middlewares and tools to integrate axum + tracing + opentelemetry

## Project showcase

- [HomeDisk](https://github.com/MedzikUser/HomeDisk): ☁️ Fast, lightweight and Open Source local cloud for your data.
- [Houseflow](https://github.com/gbaranski/houseflow): House automation platform written in Rust.
- [JWT Auth](https://github.com/Z4RX/axum_jwt_example): JWT auth service for educational purposes.
- [ROAPI](https://github.com/roapi/roapi): Create full-fledged APIs for static datasets without writing a single line of code.
- [notify.run](https://github.com/notify-run/notify-run-rs): HTTP-to-WebPush relay for sending desktop/mobile notifications to yourself, written in Rust.
- [turbo.fish](https://turbo.fish/) ([repository](https://github.com/jplatte/turbo.fish)): Find out for yourself 😉
- [Book Management](https://github.com/lz1998/axum-book-management): CRUD system of book-management with ORM and JWT for educational purposes.
- [realworld-axum-sqlx](https://github.com/launchbadge/realworld-axum-sqlx): A Rust implementation of the [Realworld] demo app spec using Axum and [SQLx].
- [Rustapi](https://github.com/ndelvalle/rustapi): RESTful API template using MongoDB
- [Jotsy](https://github.com/ohsayan/jotsy): Self-hosted notes app powered by Skytable, Axum and Tokio
- [Svix](https://www.svix.com) ([repository](https://github.com/svix/svix-webhooks)): Enterprise-ready webhook service
- [emojied](https://emojied.net) ([repository](https://github.com/sekunho/emojied)): Shorten URLs to emojis!
- [CLOMonitor](https://clomonitor.io) ([repository](https://github.com/cncf/clomonitor)): Checks open source projects repositories to verify they meet certain best practices.
- [Pinging.net](https://www.pinging.net) ([repository](https://github.com/benhansenslc/pinging)): A new way to check and monitor your internet connection.
- [wastebin](https://github.com/matze/wastebin): A minimalist pastebin service.
- [sandbox_axum_observability](https://github.com/davidB/sandbox_axum_observability) A Sandbox/showcase project to experiment axum and observability (tracing, opentelemetry, jaeger, grafana tempo,...)
- [axum_admin](https://github.com/lingdu1234/axum_admin): An admin panel built with **axum**, Sea-orm and Vue 3.

[Realworld]: https://github.com/gothinkster/realworld
[SQLx]: https://github.com/launchbadge/sqlx

## Tutorials

- [Rust on Nails](https://cloak.software/blog/rust-on-nails/): A full stack architecture for Rust web applications (uses Axum)
- [axum-tutorial] ([website][axum-tutorial-website]): Axum web framework tutorial for beginners.
- [demo-rust-axum]: Demo of Rust and axum web framework
- [Introduction to axum (talk)]: Talk about axum from the Copenhagen Rust Meetup.
- [Getting Started with Axum](https://carlosmv.hashnode.dev/getting-started-with-axum-rust): Axum tutorial, GET, POST endpoints and serving files.

[axum-tutorial]: https://github.com/programatik29/axum-tutorial
[axum-tutorial-website]: https://programatik29.github.io/axum-tutorial/
[demo-rust-axum]: https://github.com/joelparkerhenderson/demo-rust-axum
[Introduction to axum (talk)]: https://www.youtube.com/watch?v=ETdmhh7OQpA
[Getting Started witn Axum]: https://carlosmv.hashnode.dev/getting-started-with-axum-rust
