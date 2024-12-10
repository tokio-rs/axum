//! Additional types for generating responses.

#[cfg(feature = "erased-json")]
mod erased_json;

#[cfg(feature = "attachment")]
mod attachment;

#[cfg(feature = "multipart")]
pub mod multiple;

#[cfg(feature = "error-response")]
mod error_response;

#[cfg(feature = "file-stream")]
/// Module for handling file streams.
pub mod file_stream;

#[cfg(feature = "file-stream")]
pub use file_stream::FileStream;

#[cfg(feature = "error-response")]
pub use error_response::InternalServerError;

#[cfg(feature = "erased-json")]
pub use erased_json::ErasedJson;

/// _not_ public API
#[cfg(feature = "erased-json")]
#[doc(hidden)]
pub use erased_json::private as __private_erased_json;

#[cfg(feature = "json-lines")]
#[doc(no_inline)]
pub use crate::json_lines::JsonLines;

#[cfg(feature = "attachment")]
pub use attachment::Attachment;

macro_rules! mime_response {
    (
        $(#[$m:meta])*
        $ident:ident,
        $mime:ident,
    ) => {
        mime_response! {
            $(#[$m])*
            $ident,
            mime::$mime.as_ref(),
        }
    };

    (
        $(#[$m:meta])*
        $ident:ident,
        $mime:expr,
    ) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug)]
        #[must_use]
        pub struct $ident<T>(pub T);

        impl<T> axum::response::IntoResponse for $ident<T>
        where
            T: axum::response::IntoResponse,
        {
            fn into_response(self) -> axum::response::Response {
                (
                    [(
                        http::header::CONTENT_TYPE,
                        http::HeaderValue::from_static($mime),
                    )],
                    self.0,
                )
                    .into_response()
            }
        }

        impl<T> From<T> for $ident<T> {
            fn from(inner: T) -> Self {
                Self(inner)
            }
        }
    };
}

mime_response! {
    /// A JavaScript response.
    ///
    /// Will automatically get `Content-Type: application/javascript; charset=utf-8`.
    JavaScript,
    APPLICATION_JAVASCRIPT_UTF_8,
}

mime_response! {
    /// A CSS response.
    ///
    /// Will automatically get `Content-Type: text/css; charset=utf-8`.
    Css,
    TEXT_CSS_UTF_8,
}

mime_response! {
    /// A WASM response.
    ///
    /// Will automatically get `Content-Type: application/wasm`.
    Wasm,
    "application/wasm",
}

#[cfg(feature = "typed-header")]
#[doc(no_inline)]
pub use crate::typed_header::TypedHeader;
