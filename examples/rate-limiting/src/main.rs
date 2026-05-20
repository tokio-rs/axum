//! Rate limiting requests with a custom middleware.
//!
//! This example implements a per-client (per-IP) token-bucket rate limiter as
//! an [`axum::middleware::from_fn_with_state`] middleware, backed by nothing
//! but a shared `Arc<Mutex<..>>`. Each client gets a bucket that holds up to
//! `BURST` tokens and refills at `REFILL_PER_SECOND`; a request that finds an
//! empty bucket is rejected with `429 Too Many Requests` and a `Retry-After`
//! header.
//!
//! `tower`'s `RateLimitLayer` is deliberately *not* used here. It keeps its
//! state inside the service, so once the service is cloned — which axum does
//! per connection — each clone gets an independent limit and the cap is never
//! actually enforced (see <https://github.com/tokio-rs/axum/issues/2634>).
//! Keeping the state in an `Arc` shared by every clone is what makes this
//! middleware work. For production use, a dedicated crate such as
//! `tower-governor` is recommended.
//!
//! Run with
//!
//! ```not_rust
//! cargo run -p example-rate-limiting
//! ```
//!
//! Then send a burst of requests and watch the excess ones get rejected:
//!
//! ```not_rust
//! for i in $(seq 1 10); do curl -s -o /dev/null -w "%{http_code}\n" 127.0.0.1:3000/ & done; wait
//! ```

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Maximum number of requests a client may burst before being limited.
const BURST: f64 = 5.0;

/// Steady-state rate, in requests per second, each client is refilled at.
const REFILL_PER_SECOND: f64 = 1.0;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let limiter = RateLimiter::default();

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .layer(middleware::from_fn_with_state(limiter, rate_limit));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    // `into_make_service_with_connect_info` makes the peer address available
    // to the middleware via the `ConnectInfo` extractor.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await;
}

/// Middleware that applies the [`RateLimiter`] to every request, keyed by the
/// client's IP address.
async fn rate_limit(
    State(limiter): State<RateLimiter>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    match limiter.check(addr.ip()) {
        Ok(()) => next.run(req).await,
        Err(retry_after) => {
            tracing::debug!(client = %addr.ip(), "rate limited");
            (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, retry_after.to_string())],
                "Too many requests, slow down\n",
            )
                .into_response()
        }
    }
}

/// A token-bucket rate limiter shared across all requests.
///
/// Cloning is cheap and, crucially, every clone points at the *same* shared
/// state. That is what lets axum clone the middleware per connection without
/// each clone getting its own independent limit.
#[derive(Clone, Default)]
struct RateLimiter {
    buckets: Arc<Mutex<HashMap<IpAddr, Bucket>>>,
}

impl RateLimiter {
    /// Accounts for a request from `client`, returning `Ok` if it is allowed
    /// or `Err(retry_after)` — seconds to wait — if the client is limited.
    fn check(&self, client: IpAddr) -> Result<(), u64> {
        let now = Instant::now();

        // The guard is dropped at the end of this function, so the lock is
        // never held across the `.await` in `rate_limit`.
        let mut buckets = self.buckets.lock().unwrap();
        let bucket = buckets.entry(client).or_insert(Bucket {
            tokens: BURST,
            last_refill: now,
        });

        // Refill the bucket for the time elapsed since the previous request.
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * REFILL_PER_SECOND).min(BURST);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            // Seconds until the bucket holds a whole token again.
            let wait = (1.0 - bucket.tokens) / REFILL_PER_SECOND;
            Err(wait.ceil() as u64)
        }
    }
}

/// A single client's token bucket.
///
/// Note: for brevity this example never evicts entries, so the map grows with
/// the number of distinct clients. A real deployment would prune buckets that
/// have been idle (full) for a while.
struct Bucket {
    /// Tokens currently available; one is spent per allowed request.
    tokens: f64,
    /// When `tokens` was last brought up to date.
    last_refill: Instant,
}
