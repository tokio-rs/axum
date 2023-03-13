//! Additional extractors.

mod cached;

#[cfg(feature = "form")]
mod form;

#[cfg(feature = "cookie")]
pub mod cookie;

#[cfg(feature = "query")]
mod query;

#[cfg(feature = "multipart")]
pub mod multipart;

mod with_rejection;

pub use self::cached::Cached;

#[cfg(feature = "cookie")]
pub use self::cookie::CookieJar;

#[cfg(feature = "cookie-private")]
pub use self::cookie::PrivateCookieJar;

#[cfg(feature = "cookie-signed")]
pub use self::cookie::SignedCookieJar;

#[cfg(feature = "form")]
pub use self::form::{Form, FormRejection};

#[cfg(feature = "query")]
pub use self::query::{Query, QueryRejection};

#[cfg(feature = "multipart")]
pub use self::multipart::Multipart;

#[cfg(feature = "json-lines")]
#[doc(no_inline)]
pub use crate::json_lines::JsonLines;

#[cfg(feature = "typed-header")]
#[doc(no_inline)]
pub use crate::typed_header::TypedHeader;

pub use self::with_rejection::WithRejection;
