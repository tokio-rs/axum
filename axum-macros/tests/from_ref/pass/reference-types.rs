#![deny(noop_method_call)]

use axum_macros::FromRef;

#[derive(FromRef)]
struct State {
    inner: &'static str,
}

#[derive(FromRef)]
struct TupleState(&'static str);

fn main() {}
