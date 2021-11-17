//! Additional types for generating responses.

#[cfg(feature = "erased-json")]
mod erased_json;

#[cfg(feature = "erased-json")]
pub use erased_json::ErasedJson;
