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
- [axum_session](https://github.com/AscendingCreations/AxumSessions): Database persistent sessions like pythons flask_sessionstore for Axum.
- [axum_session_auth](https://github.com/AscendingCreations/AxumSessionsAuth): Persistent session based user login with rights management for Axum.
- [axum-auth](https://crates.io/crates/axum-auth): High-level http auth extractors for axum.
- [axum-keycloak-auth](https://github.com/lpotthast/axum-keycloak-auth): Protect axum routes with a JWT emitted by Keycloak.
- [axum-tungstenite](https://github.com/davidpdrsn/axum-tungstenite): WebSocket connections for axum directly using tungstenite
- [axum-jrpc](https://github.com/0xdeafbeef/axum-jrpc): Json-rpc extractor for axum
- [axum-tracing-opentelemetry](https://crates.io/crates/axum-tracing-opentelemetry): Middlewares and tools to integrate axum + tracing + opentelemetry
- [svelte-axum-project](https://github.com/jbertovic/svelte-axum-project): Template and example for Svelte frontend app with Axum as backend
- [axum-streams](https://github.com/abdolence/axum-streams-rs): Streaming HTTP body with different formats: JSON, CSV, Protobuf.
- [axum-template](https://github.com/Altair-Bueno/axum-template): Layers, extractors and template engine wrappers for axum based Web MVC applications
- [axum-template](https://github.com/janos-r/axum-template): GraphQL and REST API, SurrealDb, JWT auth, direct error handling, request logs
- [axum-guard-logic](https://github.com/sjud/axum_guard_logic): Use AND/OR logic to extract types and check their values against `Service` inputs.
- [axum-casbin-auth](https://github.com/casbin-rs/axum-casbin-auth): Casbin access control middleware for axum framework
- [aide](https://docs.rs/aide): Code-first Open API documentation generator with [axum integration](https://docs.rs/aide/latest/aide/axum/index.html).
- [axum-typed-routing](https://docs.rs/axum-typed-routing/latest/axum_typed_routing/): Statically typed routing macros with OpenAPI generation using aide.
- [axum-jsonschema](https://docs.rs/axum-jsonschema/): A `Json<T>` extractor that does JSON schema validation of requests.
- [axum-login](https://docs.rs/axum-login): Session-based user authentication for axum.
- [axum-csrf-sync-pattern](https://crates.io/crates/axum-csrf-sync-pattern): A middleware implementing CSRF STP for AJAX backends and API endpoints.
- [axum-otel-metrics](https://github.com/ttys3/axum-otel-metrics/): A axum OpenTelemetry Metrics middleware with prometheus exporter supported.
- [jwt-authorizer](https://crates.io/crates/jwt-authorizer): JWT authorization layer for axum (oidc discovery, validation options, claims extraction, etc.) 
- [axum-typed-multipart](https://crates.io/crates/axum_typed_multipart): Type safe wrapper for `axum::extract::Multipart`.
- [tower-governor](https://crates.io/crates/tower_governor): A Tower service and layer that provides a rate-limiting backend by [governor](https://crates.io/crates/governor)
- [axum-restful](https://github.com/gongzhengyang/axum-restful): A restful framework based on axum and sea-orm, inspired by django-rest-framework.
- [springtime-web-axum](https://crates.io/crates/springtime-web-axum): A web framework built on Springtime and axum, leveraging dependency injection for easy app development.
- [rust-axum-with-google-oauth](https://github.com/randommm/rust-axum-with-google-oauth): website template for Google OAuth authentication on Axum, using SQLite with SQLx or MongoDB and MiniJinja.
- [axum-htmx](https://github.com/robertwayne/axum-htmx): Htmx extractors and request guards for axum. 
- [axum-prometheus](https://github.com/ptrskay3/axum-prometheus): A middleware library to collect HTTP metrics for axum applications, compatible with all [metrics.rs](https://metrics.rs) exporters.
- [axum-valid](https://github.com/gengteng/axum-valid): Extractors for data validation using validator, garde, and validify.
- [tower-sessions](https://github.com/maxcountryman/tower-sessions): Sessions as a `tower` and `axum` middleware.
- [shuttle](https://github.com/shuttle-hq/shuttle): Build & ship backends without writing any infrastructure files. Now with Axum support.
- [socketioxide](https://github.com/totodore/socketioxide): An easy to use socket.io server implementation working as a `tower` layer/service. 
- [axum-serde](https://github.com/gengteng/axum-serde): Provides multiple serde-based extractors / responses, also offers a macro to easily customize serde-based extractors / responses.
- [loco.rs](https://github.com/loco-rs/loco): A full stack Web and API productivity framework similar to Rails, based on Axum.
- [axum-test](https://crates.io/crates/axum-test): High level library for writing Cargo tests that run against Axum.
- [axum-messages](https://github.com/maxcountryman/axum-messages): One-time notification messages for Axum.
- [spring-rs](https://github.com/spring-rs/spring-rs): spring-rs is a microservice framework written in rust inspired by java's spring-boot, based on axum
- [zino](https://github.com/zino-rs/zino): Zino is a next-generation framework for composable applications which provides full integrations with axum.
- [axum-rails-cookie](https://github.com/endoze/axum-rails-cookie): Extract rails session cookies in axum based apps.
- [axum-ws-broadcaster](https://github.com/Necoo33/axum-ws-broadcaster): A broadcasting liblary for both [axum-typed-websockets](https://crates.io/crates/axum-typed-websockets) and `axum::extract::ws`.
- [axum-negotiate-layer](https://github.com/2ndDerivative/axum-negotiate-layer) Middleware/Layer for Kerberos/NTLM "Negotiate" authentication.

## Project showcase

- [HomeDisk](https://github.com/MedzikUser/HomeDisk): ☁️ Fast, lightweight and Open Source local cloud for your data.
- [Houseflow](https://github.com/gbaranski/houseflow): House automation platform written in Rust.
- [JWT Auth](https://github.com/Z4RX/axum_jwt_example): JWT auth service for educational purposes.
- [ROAPI](https://github.com/roapi/roapi): Create full-fledged APIs for static datasets without writing a single line of code.
- [notify.run](https://github.com/notify-run/notify-run-rs): HTTP-to-WebPush relay for sending desktop/mobile notifications to yourself, written in Rust.
- [turbo.fish](https://turbo.fish/) ([repository](https://github.com/jplatte/turbo.fish)): Find out for yourself 😉
- [Book Management](https://github.com/lz1998/axum-book-management): CRUD system of book-management with ORM and JWT for educational purposes.
- [realworld-axum-sqlx](https://github.com/launchbadge/realworld-axum-sqlx): A Rust implementation of the [Realworld] demo app spec using Axum and [SQLx].
  See https://github.com/davidpdrsn/realworld-axum-sqlx for a fork with up to date dependencies.
- [Rustapi](https://github.com/ndelvalle/rustapi): RESTful API template using MongoDB
- [axum-postgres-template](https://github.com/koskeller/axum-postgres-template): Production-ready Axum + PostgreSQL application template
- [RUSTfulapi](https://github.com/robatipoor/rustfulapi): Reusable template for building REST Web Services in Rust. Uses Axum HTTP web framework and SeaORM.
- [Jotsy](https://github.com/ohsayan/jotsy): Self-hosted notes app powered by Skytable, Axum and Tokio
- [Svix](https://www.svix.com) ([repository](https://github.com/svix/svix-webhooks)): Enterprise-ready webhook service
- [emojied](https://emojied.net) ([repository](https://github.com/sekunho/emojied)): Shorten URLs to emojis!
- [CLOMonitor](https://clomonitor.io) ([repository](https://github.com/cncf/clomonitor)): Checks open source projects repositories to verify they meet certain best practices.
- [Pinging.net](https://www.pinging.net) ([repository](https://github.com/benhansenslc/pinging)): A new way to check and monitor your internet connection.
- [wastebin](https://github.com/matze/wastebin): A minimalist pastebin service.
- [sandbox_axum_observability](https://github.com/davidB/sandbox_axum_observability) A Sandbox/showcase project to experiment axum and observability (tracing, opentelemetry, jaeger, grafana tempo,...)
- [axum_admin](https://github.com/lingdu1234/axum_admin): An admin panel built with **axum**, Sea-orm and Vue 3.
- [rgit](https://git.inept.dev/~doyle/rgit.git/about): A blazingly fast Git repository browser, compatible with- and heavily inspired by cgit.
- [Petclinic](https://github.com/danipardo/petclinic): A port of Spring Framework's Petclinic showcase project to Axum
- [axum-middleware-example](https://github.com/casbin-rs/axum-middleware-example): A authorization application using Axum-web, Casbin and Diesel, with JWT support.
- [circleci-hook](https://github.com/DavidS/circleci-hook): Translate CircleCI WebHooks to OpenTelemetry traces to improve your test insights. Add detail with otel-cli to capture individual commands. Use the TRACEPARENT integration to add details from your tests.
- [lishuuro.org](https://github.com/uros-5/backend-lishuuro): Small chess variant server that uses Rust as backend(Axum framework).
- [freedit](https://github.com/freedit-org/freedit): A forum powered by rust. 
- [axum-http-auth-example](https://github.com/i0n/axum-http-auth-example): Axum http auth example using postgres and redis. 
- [Deaftone](https://github.com/Deaftone/Deaftone): Lightweight music server. With a clean and simple API 
- [dropit](https://github.com/scotow/dropit): Temporary file hosting.
- [cobrust](https://github.com/scotow/cobrust): Multiplayer web based snake game.
- [meta-cross](https://github.com/scotow/meta-cross): Tweaked version of Tic-Tac-Toe.
- [httq](https://github.com/scotow/httq) HTTP to MQTT trivial proxy.
- [Pods-Blitz](https://pods-blitz.org) Self-hosted podcast publisher. Uses the crates axum-login, password-auth, sqlx and handlebars (for HTML templates). 
- [ReductStore](https://github.com/reductstore/reductstore): A time series database for storing and managing large amounts of blob data
- [randoku](https://github.com/stchris/randoku): A tiny web service which generates random numbers and shuffles lists randomly
- [sero](https://github.com/clowzed/sero): Host static sites with custom subdomains as surge.sh does. But with full control and cool new features. (axum, sea-orm, postgresql)
- [Hatsu](https://github.com/importantimport/hatsu): 🩵 Self-hosted & Fully-automated ActivityPub Bridge for Static Sites.
- [Mini RPS](https://github.com/marcodpt/minirps): Mini reverse proxy server, HTTPS, CORS, static file hosting and template engine (minijinja).

[Realworld]: https://github.com/gothinkster/realworld
[SQLx]: https://github.com/launchbadge/sqlx

## Tutorials

- [Rust on Nails](https://rust-on-nails.com/): A full stack architecture for Rust web applications (uses Axum)
- [axum-tutorial] ([website][axum-tutorial-website]): Axum web framework tutorial for beginners.
- [demo-rust-axum]: Demo of Rust and axum web framework
- [Introduction to axum (talk)]: Talk about axum from the Copenhagen Rust Meetup.
- [Getting Started with Axum]: Axum tutorial, GET, POST endpoints and serving files.
- [Using Rust, Axum, PostgreSQL, and Tokio to build a Blog]
- [Introduction to axum]: YouTube playlist
- [Rust Axum Full Course]: YouTube video
- [Deploying Axum projects with Shuttle]
- [API Development with Rust](https://rust-api.dev/docs/front-matter/preface/): REST APIs based on Axum

[axum-tutorial]: https://github.com/programatik29/axum-tutorial
[axum-tutorial-website]: https://programatik29.github.io/axum-tutorial/
[demo-rust-axum]: https://github.com/joelparkerhenderson/demo-rust-axum
[Introduction to axum (talk)]: https://www.youtube.com/watch?v=ETdmhh7OQpA
[Getting Started with Axum]: https://carlosmv.hashnode.dev/getting-started-with-axum-rust
[Using Rust, Axum, PostgreSQL, and Tokio to build a Blog]: https://spacedimp.com/blog/using-rust-axum-postgresql-and-tokio-to-build-a-blog/
[Introduction to axum]: https://www.youtube.com/playlist?list=PLrmY5pVcnuE-_CP7XZ_44HN-mDrLQV4nS
[Rust Axum Full Course]: https://www.youtube.com/watch?v=XZtlD_m59sM
[Deploying Axum projects with Shuttle]: https://docs.shuttle.rs/examples/axum
[Building a SaaS with Rust & Next.js](https://joshmo.bearblog.dev/lets-build-a-saas-with-rust/) A tutorial for combining Next.js with Rust via Axum to make a SaaS.
