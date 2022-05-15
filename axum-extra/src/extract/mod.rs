//! Additional extractors.

mod cached;

#[cfg(feature = "form")]
mod form;

#[cfg(feature = "cookie")]
pub mod cookie;

pub use self::cached::Cached;

#[cfg(feature = "cookie")]
pub use self::cookie::CookieJar;

#[cfg(feature = "cookie-private")]
pub use self::cookie::PrivateCookieJar;

#[cfg(feature = "cookie-signed")]
pub use self::cookie::SignedCookieJar;

#[cfg(feature = "form")]
pub use self::form::Form;
