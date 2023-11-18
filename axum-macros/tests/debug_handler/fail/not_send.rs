use axum_macros::debug_handler;

#[debug_handler]
async fn handler() {
    let _rc = std::rc::Rc::new(());
    async {}.await;
}

fn main() {}
