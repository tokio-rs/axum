use axum_macros::debug_handler;

#[debug_handler(with(T = String))]
async fn handler<T>() -> Result<T, ()> {
    Err(())
}

fn main() {}
