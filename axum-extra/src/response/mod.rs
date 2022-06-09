//! Additional types for generating responses.

#[cfg(feature = "erased-json")]
mod erased_json;

#[cfg(feature = "erased-json")]
pub use erased_json::ErasedJson;

#[cfg(feature = "file-response")]
mod file_response;

#[cfg(feature = "file-response")]
pub use file_response::FileResponse;
