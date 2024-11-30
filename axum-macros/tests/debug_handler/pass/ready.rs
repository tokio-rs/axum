use axum_macros::debug_handler;
use std::future::{ready, Ready};

#[debug_handler]
fn handler() -> Ready<()> {
    ready(())
}

fn main() {}
