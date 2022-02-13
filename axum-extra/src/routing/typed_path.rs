use std::borrow::Cow;

/// TODO
pub trait TypedPath {
    /// TODO
    const PATH: &'static str;

    /// TODO
    fn path(&self) -> Cow<'static, str>;
}
