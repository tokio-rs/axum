//! Example for using associated methods as handlers or middleware.
//!
//! Run with
//!
//! ```not_rust
//! cargo run -p example-closures
//! ```

use axum::{
    extract::{Query, Request, State},
    middleware::{from_fn, Next},
    response::IntoResponse,
    routing::get,
    Router,
};
use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc},
};

struct AppState {
    version: String,
}

type S = Arc<AppState>;

struct Bar {
    id: AtomicU64,
    prefix: String,
}

struct Foo {
    bar: Bar,
}

impl Foo {
    async fn a(self: Arc<Self>, q: Query<HashMap<String, String>>) -> impl IntoResponse {
        let msg = q.get("msg").cloned().unwrap_or("world".to_string());
        format!("{} {}", self.bar.prefix, msg)
    }

    async fn b(self: Arc<Self>, state: State<S>, mut req: Request) -> impl IntoResponse {
        let id = req.extensions_mut().remove::<u64>();

        if let Some(id) = id {
            format!("{}: id {}", state.version, id)
        } else {
            format!("{}: no id", state.version)
        }
    }

    async fn c(self: Arc<Self>, mut req: Request, next: Next) -> impl IntoResponse {
        let id = self
            .bar
            .id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        req.extensions_mut().insert(id);

        next.run(req).await
    }
}

macro_rules! c {
    ($arc:expr, |$($param:ident),*| $method:ident) => {{
        let s = Arc::clone(&$arc);
        move |$($param),*| {
            let s = Arc::clone(&s); // only needed for middleware
            async move { s.$method($($param),*).await }
        }
    }};
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState {
        version: "v1".to_string(),
    });

    let f = Foo {
        bar: Bar {
            id: AtomicU64::new(0),
            prefix: "Hello".to_string(),
        },
    };

    let f = Arc::new(f);

    let app = Router::new()
        .route("/a", get(c!(f, |q| a)))
        .route("/b", get(c!(f, |s, q| b)))
        .layer(from_fn(c!(f, |r, n| c)))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
