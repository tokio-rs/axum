use bytes::Bytes;
use pin_project_lite::pin_project;
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PercentDecodedByteStr(ByteStr);

impl PercentDecodedByteStr {
    pub(crate) fn new<S>(s: S) -> Option<Self>
    where
        S: AsRef<str>,
    {
        percent_encoding::percent_decode(s.as_ref().as_bytes())
            .decode_utf8()
            .ok()
            .map(|decoded| Self(ByteStr::new(decoded)))
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Deref for PercentDecodedByteStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

pin_project! {
    #[project = EitherProj]
    pub(crate) enum Either<A, B> {
        A { #[pin] inner: A },
        B { #[pin] inner: B },
    }
}
