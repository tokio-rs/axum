//! Additional middleware.

#[cfg(feature = "opentelemetry")]
pub mod opentelemetry;

#[cfg(feature = "opentelemetry")]
pub use self::opentelemetry::opentelemetry_tracing_layer;
