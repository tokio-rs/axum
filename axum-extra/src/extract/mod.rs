//! Additional extractors.

pub mod rejection;

#[cfg(feature = "cached")]
mod cached;

#[cfg(feature = "with-rejection")]
mod with_rejection;

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

#[cfg(feature = "cached")]
pub use self::cached::Cached;

#[cfg(feature = "with-rejection")]
pub use self::with_rejection::WithRejection;

#[cfg(feature = "cookie")]
pub use self::cookie::CookieJar;

#[cfg(feature = "cookie-private")]
pub use self::cookie::PrivateCookieJar;

#[cfg(feature = "cookie-signed")]
pub use self::cookie::SignedCookieJar;

#[cfg(feature = "form")]
#[allow(deprecated)]
pub use self::form::{Form, FormRejection};

#[cfg(feature = "query")]
pub use self::query::OptionalQuery;
#[cfg(feature = "query")]
#[allow(deprecated)]
pub use self::query::{OptionalQueryRejection, Query, QueryRejection};

#[cfg(feature = "multipart")]
pub use self::multipart::Multipart;

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
