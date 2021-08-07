use bytes::Bytes;
use std::ops::Deref;

/// A string like type backed by `Bytes` making it cheap to clone.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ByteStr(Bytes);

impl Deref for ByteStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl ByteStr {
    pub(crate) fn new<S>(s: S) -> Self
    where
        S: AsRef<str>,
    {
        Self(Bytes::copy_from_slice(s.as_ref().as_bytes()))
    }

    pub(crate) fn as_str(&self) -> &str {
        // `ByteStr` can only be constructed from strings which are always valid
        // utf-8 so this wont panic.
        std::str::from_utf8(&self.0).unwrap()
    }
}
