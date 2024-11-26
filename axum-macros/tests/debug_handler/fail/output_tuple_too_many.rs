use axum::response::AppendHeaders;

#[axum::debug_handler]
async fn handler() -> (
    axum::http::StatusCode,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    AppendHeaders<[(axum::http::HeaderName, &'static str); 1]>,
    axum::http::StatusCode,
) {
    panic!()
}

fn main() {}
