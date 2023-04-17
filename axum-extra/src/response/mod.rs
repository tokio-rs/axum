//! Additional types for generating responses.

#[cfg(feature = "erased-json")]
mod erased_json;

#[cfg(feature = "erased-json")]
pub use erased_json::ErasedJson;

#[cfg(feature = "json-lines")]
#[doc(no_inline)]
pub use crate::json_lines::JsonLines;

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
    /// A HTML response.
    ///
    /// Will automatically get `Content-Type: text/html; charset=utf-8`.
    Html,
    TEXT_HTML_UTF_8,
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
