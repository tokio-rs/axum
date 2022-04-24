use axum::routing::get;

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}

pub struct ApiService;

impl ApiService {
    pub fn build() -> axum::Router {
        axum::Router::new().route("/", get(root).post(root))
    }
}
