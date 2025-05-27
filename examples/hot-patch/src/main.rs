use axum::{response::Html, routing::get, Router};

#[tokio::main]
async fn main() {
    dioxus_devtools::connect_subsecond();
    let app = Router::new().route("/", get(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler() -> Html<&'static str> {
    dioxus_devtools::subsecond::call(|| {
        Html("<h1>Hello, World!</h1>")
    })
}
