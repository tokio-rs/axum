//! Additional middleware.

#[cfg(feature = "opentelemetry")]
pub mod opentelemetry;

pub use self::opentelemetry::opentelemtry_tracing_layer;
