//! Additional extractors.

mod cached;
mod host;
mod optional_path;
pub mod rejection;
mod with_rejection;

/// Private mod, public trait trick
mod spoof {
    pub trait FromSpoofableRequestParts<S>: Sized {
        type Rejection: axum::response::IntoResponse;

        fn from_request_parts(
            parts: &mut axum::http::request::Parts,
            state: &S,
        ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send;
    }
}

/// Wrap spoofable extractor
#[derive(Debug)]
pub struct Spoofable<E>(pub E);

/// Allow `Spoofable` to be used with spoofable extractors in handlers
impl<S, E> FromRequestParts<S> for Spoofable<E>
where
    E: spoof::FromSpoofableRequestParts<S>,
    S: Sync,
{
    type Rejection = E::Rejection;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        E::from_request_parts(parts, state).await.map(Spoofable)
    }
}

#[cfg(feature = "form")]
mod form;

#[cfg(feature = "cookie")]
pub mod cookie;

#[cfg(feature = "json-deserializer")]
mod json_deserializer;

#[cfg(feature = "query")]
mod query;

#[cfg(feature = "multipart")]
pub mod multipart;

#[cfg(feature = "scheme")]
mod scheme;

use axum::extract::FromRequestParts;

pub use self::{
    cached::Cached, host::Host, optional_path::OptionalPath, with_rejection::WithRejection,
};

#[cfg(feature = "cookie")]
pub use self::cookie::CookieJar;

#[cfg(feature = "cookie-private")]
pub use self::cookie::PrivateCookieJar;

#[cfg(feature = "cookie-signed")]
pub use self::cookie::SignedCookieJar;

#[cfg(feature = "form")]
pub use self::form::{Form, FormRejection};

#[cfg(feature = "query")]
pub use self::query::{OptionalQuery, OptionalQueryRejection, Query, QueryRejection};

#[cfg(feature = "multipart")]
pub use self::multipart::Multipart;

#[cfg(feature = "scheme")]
#[doc(no_inline)]
pub use self::scheme::{Scheme, SchemeMissing};

#[cfg(feature = "json-deserializer")]
pub use self::json_deserializer::{
    JsonDataError, JsonDeserializer, JsonDeserializerRejection, JsonSyntaxError,
    MissingJsonContentType,
};

#[cfg(feature = "json-lines")]
#[doc(no_inline)]
pub use crate::json_lines::JsonLines;

#[cfg(feature = "typed-header")]
#[doc(no_inline)]
pub use crate::typed_header::TypedHeader;
