# Required dependencies

To use axum there are a few dependencies you have pull in as well:

```toml
[dependencies]
axum = "<latest-version>"
hyper = { version = "<latest-version>", features = ["full"] }
tokio = { version = "<latest-version>", features = ["full"] }
tower = "<latest-version>"
```

The `"full"` feature for hyper and tokio isn't strictly necessary but its
the easiest way to get started.

Note that [`hyper::Server`] is re-exported by axum so if thats all you need
then you don't have to explicitly depend on hyper.

Tower isn't strictly necessary either but helpful for testing. See the
testing example in the repo to learn more about testing axum apps.
