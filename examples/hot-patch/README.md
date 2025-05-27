# auto-reload

This example shows how you can set up a development environment with
hot-patching for certain code paths with subsecond, this allows you to change
your code without restarting it.

It currently uses the git version of dioxus-cli and both the library and the cli
must be the same version.

## Setup

```sh
cargo install dioxus-cli --git https://github.com/DioxusLabs/dioxus --rev 77d10a4
```

## Running

```sh
dx serve --hot-patch
```

