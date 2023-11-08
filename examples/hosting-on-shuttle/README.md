# Hosting on Shuttle

<img width="300" src="https://raw.githubusercontent.com/shuttle-hq/shuttle/master/assets/logo-rectangle-transparent.png"/>

> [**Shuttle**](https://www.shuttle.rs) is a Rust-native cloud development platform that lets you deploy your Rust apps for free.

Shuttle has out-of-the-box support for Axum. You can follow these steps to run the example:

1. Install `cargo-shuttle`:

```sh
cargo install cargo-shuttle
```

2. Run the example locally:

```sh
cargo shuttle run --working-directory examples/hosting-on-shuttle
```

If you want to create a project and deploy a service:

1. Log in into Shuttle console:

```sh
cargo shuttle login
```

2. Create a project:

```sh
cargo shuttle project start
```

3. Deploy! 🚀

```sh
cargo shuttle deploy
```

Check out the complete Axum examples [here](https://github.com/shuttle-hq/shuttle-examples/tree/main/axum).
