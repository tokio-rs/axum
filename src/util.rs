use bytes::Bytes;

/// A string like type backed by `Bytes` making it cheap to clone.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ByteStr(Bytes);

impl ByteStr {
    pub(crate) fn new<S>(s: S) -> Self
    where
        S: AsRef<str>,
    {
        Self(Bytes::copy_from_slice(s.as_ref().as_bytes()))
    }

    #[allow(unsafe_code)]
    pub(crate) fn as_str(&self) -> &str {
        // SAFETY: `ByteStr` can only be constructed from strings which are
        // always valid utf-8.
        unsafe { std::str::from_utf8_unchecked(&self.0) }
    }
}
