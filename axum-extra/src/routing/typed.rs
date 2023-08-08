use std::{any::type_name, fmt};

use super::sealed::Sealed;
use http::Uri;
use serde::Serialize;

/// A type safe path.
///
/// This is used to statically connect a path to its corresponding handler using
/// [`RouterExt::typed_get`], [`RouterExt::typed_post`], etc.
///
/// # Example
///
/// ```rust
/// use serde::Deserialize;
/// use axum::{Router, extract::Json};
/// use axum_extra::routing::{
///     TypedPath,
///     RouterExt, // for `Router::typed_*`
/// };
///
/// // A type safe route with `/users/:id` as its associated path.
/// #[derive(TypedPath, Deserialize)]
/// #[typed_path("/users/:id")]
/// struct UsersMember {
///     id: u32,
/// }
///
/// // A regular handler function that takes `UsersMember` as the first argument
/// // and thus creates a typed connection between this handler and the `/users/:id` path.
/// //
/// // The `TypedPath` must be the first argument to the function.
/// async fn users_show(
///     UsersMember { id }: UsersMember,
/// ) {
///     // ...
/// }
///
/// let app = Router::new()
///     // Add our typed route to the router.
///     //
///     // The path will be inferred to `/users/:id` since `users_show`'s
///     // first argument is `UsersMember` which implements `TypedPath`
///     .typed_get(users_show)
///     .typed_post(users_create)
///     .typed_delete(users_destroy);
///
/// #[derive(TypedPath)]
/// #[typed_path("/users")]
/// struct UsersCollection;
///
/// #[derive(Deserialize)]
/// struct UsersCreatePayload { /* ... */ }
///
/// async fn users_create(
///     _: UsersCollection,
///     // Our handlers can accept other extractors.
///     Json(payload): Json<UsersCreatePayload>,
/// ) {
///     // ...
/// }
///
/// async fn users_destroy(_: UsersCollection) { /* ... */ }
///
/// #
/// # let app: Router = app;
/// ```
///
/// # Using `#[derive(TypedPath)]`
///
/// While `TypedPath` can be implemented manually, it's _highly_ recommended to derive it:
///
/// ```
/// use serde::Deserialize;
/// use axum_extra::routing::TypedPath;
///
/// #[derive(TypedPath, Deserialize)]
/// #[typed_path("/users/:id")]
/// struct UsersMember {
///     id: u32,
/// }
/// ```
///
/// The macro expands to:
///
/// - A `TypedPath` implementation.
/// - A [`FromRequest`] implementation compatible with [`RouterExt::typed_get`],
/// [`RouterExt::typed_post`], etc. This implementation uses [`Path`] and thus your struct must
/// also implement [`serde::Deserialize`], unless it's a unit struct.
/// - A [`Display`] implementation that interpolates the captures. This can be used to, among other
/// things, create links to known paths and have them verified statically. Note that the
/// [`Display`] implementation for each field must return something that's compatible with its
/// [`Deserialize`] implementation.
///
/// Additionally the macro will verify the captures in the path matches the fields of the struct.
/// For example this fails to compile since the struct doesn't have a `team_id` field:
///
/// ```compile_fail
/// use serde::Deserialize;
/// use axum_extra::routing::TypedPath;
///
/// #[derive(TypedPath, Deserialize)]
/// #[typed_path("/users/:id/teams/:team_id")]
/// struct UsersMember {
///     id: u32,
/// }
/// ```
///
/// Unit and tuple structs are also supported:
///
/// ```
/// use serde::Deserialize;
/// use axum_extra::routing::TypedPath;
///
/// #[derive(TypedPath)]
/// #[typed_path("/users")]
/// struct UsersCollection;
///
/// #[derive(TypedPath, Deserialize)]
/// #[typed_path("/users/:id")]
/// struct UsersMember(u32);
/// ```
///
/// ## Percent encoding
///
/// The generated [`Display`] implementation will automatically percent-encode the arguments:
///
/// ```
/// use serde::Deserialize;
/// use axum_extra::routing::TypedPath;
///
/// #[derive(TypedPath, Deserialize)]
/// #[typed_path("/users/:id")]
/// struct UsersMember {
///     id: String,
/// }
///
/// assert_eq!(
///     UsersMember {
///         id: "foo bar".to_string(),
///     }.to_string(),
///     "/users/foo%20bar",
/// );
/// ```
///
/// ## Customizing the rejection
///
/// By default the rejection used in the [`FromRequest`] implementation will be [`PathRejection`].
///
/// That can be customized using `#[typed_path("...", rejection(YourType))]`:
///
/// ```
/// use serde::Deserialize;
/// use axum_extra::routing::TypedPath;
/// use axum::{
///     response::{IntoResponse, Response},
///     extract::rejection::PathRejection,
/// };
///
/// #[derive(TypedPath, Deserialize)]
/// #[typed_path("/users/:id", rejection(UsersMemberRejection))]
/// struct UsersMember {
///     id: String,
/// }
///
/// struct UsersMemberRejection;
///
/// // Your rejection type must implement `From<PathRejection>`.
/// //
/// // Here you can grab whatever details from the inner rejection
/// // that you need.
/// impl From<PathRejection> for UsersMemberRejection {
///     fn from(rejection: PathRejection) -> Self {
///         # UsersMemberRejection
///         // ...
///     }
/// }
///
/// // Your rejection must implement `IntoResponse`, like all rejections.
/// impl IntoResponse for UsersMemberRejection {
///     fn into_response(self) -> Response {
///         # ().into_response()
///         // ...
///     }
/// }
/// ```
///
/// The `From<PathRejection>` requirement only applies if your typed path is a struct with named
/// fields or a tuple struct. For unit structs your rejection type must implement `Default`:
///
/// ```
/// use axum_extra::routing::TypedPath;
/// use axum::response::{IntoResponse, Response};
///
/// #[derive(TypedPath)]
/// #[typed_path("/users", rejection(UsersCollectionRejection))]
/// struct UsersCollection;
///
/// #[derive(Default)]
/// struct UsersCollectionRejection;
///
/// impl IntoResponse for UsersCollectionRejection {
///     fn into_response(self) -> Response {
///         # ().into_response()
///         // ...
///     }
/// }
/// ```
///
/// [`FromRequest`]: axum::extract::FromRequest
/// [`RouterExt::typed_get`]: super::RouterExt::typed_get
/// [`RouterExt::typed_post`]: super::RouterExt::typed_post
/// [`Path`]: axum::extract::Path
/// [`Display`]: std::fmt::Display
/// [`Deserialize`]: serde::Deserialize
/// [`PathRejection`]: axum::extract::rejection::PathRejection
pub trait TypedPath: std::fmt::Display {
    /// The path with optional captures such as `/users/:id`.
    const PATH: &'static str;

    /// Convert the path into a `Uri`.
    ///
    /// # Panics
    ///
    /// The default implementation parses the required [`Display`] implementation. If that fails it
    /// will panic.
    ///
    /// Using `#[derive(TypedPath)]` will never result in a panic since it percent-encodes
    /// arguments.
    ///
    /// [`Display`]: std::fmt::Display
    fn to_uri(&self) -> Uri {
        self.to_string().parse().unwrap()
    }

    /// Add query parameters to a path.
    ///
    /// # Example
    ///
    /// ```
    /// use axum_extra::routing::TypedPath;
    /// use serde::Serialize;
    ///
    /// #[derive(TypedPath)]
    /// #[typed_path("/users")]
    /// struct Users;
    ///
    /// #[derive(Serialize)]
    /// struct Pagination {
    ///     page: u32,
    ///     per_page: u32,
    /// }
    ///
    /// let path = Users.with_query_params(Pagination {
    ///     page: 1,
    ///     per_page: 10,
    /// });
    ///
    /// assert_eq!(path.to_uri(), "/users?&page=1&per_page=10");
    /// ```
    ///
    /// # Panics
    ///
    /// If `params` doesn't support being serialized as query params [`WithQueryParams`]'s [`Display`]
    /// implementation will panic, and thus [`WithQueryParams::to_uri`] will also panic.
    ///
    /// [`WithQueryParams::to_uri`]: TypedPath::to_uri
    /// [`Display`]: std::fmt::Display
    fn with_query_params<T>(self, params: T) -> WithQueryParams<Self, T>
    where
        T: Serialize,
        Self: Sized,
    {
        WithQueryParams { path: self, params }
    }
}

/// A [`TypedPath`] with query params.
///
/// See [`TypedPath::with_query_params`] for more details.
#[derive(Debug, Clone, Copy)]
pub struct WithQueryParams<P, T> {
    path: P,
    params: T,
}

impl<P, T> fmt::Display for WithQueryParams<P, T>
where
    P: TypedPath,
    T: Serialize,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = self.path.to_string();
        if !out.contains('?') {
            out.push('?');
        }
        let mut urlencoder = form_urlencoded::Serializer::new(&mut out);
        self.params
            .serialize(serde_html_form::ser::Serializer::new(&mut urlencoder))
            .unwrap_or_else(|err| {
                panic!(
                    "failed to URL encode value of type `{}`: {}",
                    type_name::<T>(),
                    err
                )
            });
        f.write_str(&out)?;

        Ok(())
    }
}

impl<P, T> TypedPath for WithQueryParams<P, T>
where
    P: TypedPath,
    T: Serialize,
{
    const PATH: &'static str = P::PATH;
}

/// Utility trait used with [`RouterExt`] to ensure the second element of a tuple type is a
/// given type.
///
/// If you see it in type errors its most likely because the second argument to your handler doesn't
/// implement [`TypedPath`].
///
/// You normally shouldn't have to use this trait directly.
///
/// It is sealed such that it cannot be implemented outside this crate.
///
/// [`RouterExt`]: super::RouterExt
pub trait SecondElementIs<P>: Sealed {}

macro_rules! impl_second_element_is {
    ( $($ty:ident),* $(,)? ) => {
        impl<M, P, $($ty,)*> SecondElementIs<P> for (M, P, $($ty,)*)
        where
            P: TypedPath
        {}

        impl<M, P, $($ty,)*> Sealed for (M, P, $($ty,)*)
        where
            P: TypedPath
        {}

        impl<M, P, $($ty,)*> SecondElementIs<P> for (M, Option<P>, $($ty,)*)
        where
            P: TypedPath
        {}

        impl<M, P, $($ty,)*> Sealed for (M, Option<P>, $($ty,)*)
        where
            P: TypedPath
        {}

        impl<M, P, E, $($ty,)*> SecondElementIs<P> for (M, Result<P, E>, $($ty,)*)
        where
            P: TypedPath
        {}

        impl<M, P, E, $($ty,)*> Sealed for (M, Result<P, E>, $($ty,)*)
        where
            P: TypedPath
        {}
    };
}

impl_second_element_is!();
impl_second_element_is!(T1);
impl_second_element_is!(T1, T2);
impl_second_element_is!(T1, T2, T3);
impl_second_element_is!(T1, T2, T3, T4);
impl_second_element_is!(T1, T2, T3, T4, T5);
impl_second_element_is!(T1, T2, T3, T4, T5, T6);
impl_second_element_is!(T1, T2, T3, T4, T5, T6, T7);
impl_second_element_is!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_second_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_second_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_second_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_second_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_second_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_second_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_second_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
impl_second_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::TypedPath;
    use serde::Deserialize;

    #[derive(TypedPath, Deserialize)]
    #[typed_path("/users/:id")]
    struct UsersShow {
        id: i32,
    }

    #[derive(Serialize)]
    struct Params {
        foo: &'static str,
        bar: i32,
        baz: bool,
    }

    #[test]
    fn with_params() {
        let path = UsersShow { id: 1 }.with_query_params(Params {
            foo: "foo",
            bar: 123,
            baz: true,
        });

        let uri = path.to_uri();

        // according to [the spec] starting the params with `?&` is allowed specifically:
        //
        // > If bytes is the empty byte sequence, then continue.
        //
        // [the spec]: https://url.spec.whatwg.org/#urlencoded-parsing
        assert_eq!(uri, "/users/1?&foo=foo&bar=123&baz=true");
    }

    #[test]
    fn with_params_called_multiple_times() {
        let path = UsersShow { id: 1 }
            .with_query_params(Params {
                foo: "foo",
                bar: 123,
                baz: true,
            })
            .with_query_params([("qux", 1337)]);

        let uri = path.to_uri();

        assert_eq!(uri, "/users/1?&foo=foo&bar=123&baz=true&qux=1337");
    }
}
