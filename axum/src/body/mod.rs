//! HTTP body utilities.

use crate::{BoxError, Error};
use std::any::Any;

mod stream_body;

pub use self::stream_body::StreamBody;

#[doc(no_inline)]
pub use http_body::{Body as HttpBody, Empty, Full};

#[doc(no_inline)]
pub use hyper::body::Body;

#[doc(no_inline)]
pub use bytes::Bytes;

/// A boxed [`Body`] trait object.
///
/// This is used in axum as the response body type for applications. It's
/// necessary to unify multiple response bodies types into one.
pub type BoxBody = http_body::combinators::UnsyncBoxBody<Bytes, Error>;

/// Convert a [`http_body::Body`] into a [`BoxBody`].
///
/// If the given body is already a [`BoxBody`], this function will not box the body again.
pub fn boxed<B>(body: B) -> BoxBody
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    downcast_box_body(body).unwrap_or_else(|body| body.map_err(Error::new).boxed_unsync())
}

fn downcast_box_body<B: 'static>(body: B) -> Result<BoxBody, B> {
    let mut body = Some(body);
    if let Some(body) = <dyn Any>::downcast_mut::<Option<BoxBody>>(&mut body) {
        Ok(body.take().unwrap())
    } else {
        Err(body.unwrap())
    }
}

#[test]
fn body_not_double_boxed() {
    let body = Full::new(Bytes::from("hello world"));
    assert!(downcast_box_body(body.clone()).is_err());
    assert!(downcast_box_body(boxed(body.clone())).is_ok());
    assert!(downcast_box_body(boxed(boxed(body.clone()))).is_ok());
}

pub(crate) fn empty() -> BoxBody {
    boxed(http_body::Empty::new())
}
