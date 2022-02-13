#![allow(missing_docs)]

use axum::extract::{FromRequest, Path};
use serde::de::DeserializeOwned;
use std::borrow::Cow;

/// ```rust
/// use axum_macros::TypedPath;
///
/// #[derive(TypedPath)]
/// #[typed_path("/users/:id")]
/// struct UsersShow {
///     id: u32,
/// }
/// ```
pub trait TypedPath<B>: FromRequest<B> + DeserializeOwned {
    const PATH: &'static str;

    fn path(&self) -> Cow<'static, str>;
}

// pub trait FirstElementIsPath {}
// impl<P> FirstElementIsPath for (Path<P>,) {}
// impl<P, T1> FirstElementIsPath for (Path<P>, T1) {}
// impl<P, T1, T2> FirstElementIsPath for (Path<P>, T1, T2) {}
