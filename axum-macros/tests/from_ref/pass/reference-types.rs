#![deny(noop_method_call)]

use axum_macros::FromRef;

#[derive(FromRef)]
struct State {
    inner: &'static str,
}

fn main() {}
