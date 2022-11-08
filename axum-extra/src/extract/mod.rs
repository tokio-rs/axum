//! Additional extractors.

mod cached;

#[cfg(feature = "form")]
mod form;

#[cfg(feature = "cookie")]
pub mod cookie;

#[cfg(feature = "query")]
mod query;

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

#[cfg(feature = "json-lines")]
#[doc(no_inline)]
pub use crate::json_lines::JsonLines;

pub use self::with_rejection::WithRejection;
