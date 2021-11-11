use axum_debug::debug_handler;

#[debug_handler]
async fn handler() {
    let rc = std::rc::Rc::new(());
    async {}.await;
}

fn main() {}
