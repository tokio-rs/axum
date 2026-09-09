use std::{
    any::type_name,
    fmt,
    ops::{Deref, DerefMut},
};

use super::sealed::Sealed;
use http::Uri;
use serde_core::Serialize;

/// A typed route whose HTTP method is associated with a handler.
///
/// Use [`RouterExt::typed`] when the method should be inferred from this metadata together with a
/// [`TypedPath`] handler argument. The method-specific `RouterExt::typed_*` helpers remain
/// available because they choose the method explicitly and require only [`TypedPath`].
///
/// # Example
///
/// ```
/// use axum::{routing::MethodFilter, Router};
/// use axum_extra::routing::{RouterExt, TypedMethod, TypedPath};
/// use serde::Deserialize;
///
/// #[derive(TypedPath, TypedMethod, Deserialize)]
/// #[typed_path("/users/{id}")]
/// #[typed_method(MethodFilter::GET)]
/// struct GetUser {
///     id: u32,
/// }
///
/// async fn get_user(GetUser { id: _ }: GetUser) {}
///
/// let _: Router = Router::new().typed(get_user);
/// ```
///
/// A shared path can instead be wrapped in method-specific newtypes:
///
/// ```
/// use axum::Router;
/// use axum_extra::routing::{Get, Put, RouterExt, TypedPath};
/// use serde::Deserialize;
///
/// #[derive(TypedPath, Deserialize)]
/// #[typed_path("/users/{id}")]
/// struct UserPath {
///     id: u32,
/// }
///
/// async fn get_user(Get(UserPath { id: _ }): Get<UserPath>) {}
/// async fn put_user(Put(UserPath { id: _ }): Put<UserPath>) {}
///
/// let _: Router = Router::new().typed(get_user).typed(put_user);
/// ```
///
/// [`RouterExt::typed`]: super::RouterExt::typed
pub trait TypedMethod {
    /// The HTTP method used when routing this endpoint.
    const METHOD: axum::routing::MethodFilter;
}

macro_rules! typed_method_wrapper {
    ($name:ident, $method:ident) => {
        #[doc = concat!("A typed `", stringify!($method), "` method wrapper.")]
        #[repr(transparent)]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name<P>(pub P);

        impl<P> $name<P> {
            /// Unwrap the underlying typed path.
            pub fn into_inner(self) -> P {
                self.0
            }
        }

        impl<P> From<P> for $name<P> {
            fn from(path: P) -> Self {
                Self(path)
            }
        }

        impl<P> AsRef<P> for $name<P> {
            fn as_ref(&self) -> &P {
                &self.0
            }
        }

        impl<P> AsMut<P> for $name<P> {
            fn as_mut(&mut self) -> &mut P {
                &mut self.0
            }
        }

        impl<P> Deref for $name<P> {
            type Target = P;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<P> DerefMut for $name<P> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl<P> fmt::Display for $name<P>
        where
            P: fmt::Display,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl<P> TypedPath for $name<P>
        where
            P: TypedPath,
        {
            const PATH: &'static str = P::PATH;
        }

        impl<P> TypedMethod for $name<P> {
            const METHOD: axum::routing::MethodFilter = axum::routing::MethodFilter::$method;
        }

        impl<P, S> axum::extract::FromRequestParts<S> for $name<P>
        where
            P: axum::extract::FromRequestParts<S>,
            S: Send + Sync,
        {
            type Rejection = P::Rejection;

            async fn from_request_parts(
                parts: &mut axum::http::request::Parts,
                state: &S,
            ) -> Result<Self, Self::Rejection> {
                P::from_request_parts(parts, state).await.map(Self)
            }
        }
    };
}

typed_method_wrapper!(Connect, CONNECT);
typed_method_wrapper!(Delete, DELETE);
typed_method_wrapper!(Get, GET);
typed_method_wrapper!(Head, HEAD);
typed_method_wrapper!(Options, OPTIONS);
typed_method_wrapper!(Patch, PATCH);
typed_method_wrapper!(Post, POST);
typed_method_wrapper!(Put, PUT);
typed_method_wrapper!(Query, QUERY);
typed_method_wrapper!(Trace, TRACE);

/// A type safe path.
///
/// This is used to statically connect a path to its corresponding handler using
/// [`RouterExt::typed_get`], [`RouterExt::typed_post`], etc.
/// A type can also implement [`TypedMethod`]; doing so does not prevent it from being used with
/// the method-specific helpers.
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
/// // A type safe route with `/users/{id}` as its associated path.
/// #[derive(TypedPath, Deserialize)]
/// #[typed_path("/users/{id}")]
/// struct UsersMember {
///     id: u32,
/// }
///
/// // A regular handler function that takes `UsersMember` as the first argument
/// // and thus creates a typed connection between this handler and the `/users/{id}` path.
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
///     // The path will be inferred to `/users/{id}` since `users_show`'s
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
/// #[typed_path("/users/{id}")]
/// struct UsersMember {
///     id: u32,
/// }
/// ```
///
/// The macro expands to:
///
/// - A `TypedPath` implementation.
/// - A [`FromRequest`] implementation compatible with [`RouterExt::typed_get`],
///   [`RouterExt::typed_post`], etc. This implementation uses [`Path`] and thus your struct must
///   also implement [`serde::Deserialize`], unless it's a unit struct.
/// - A [`Display`] implementation that interpolates the captures. This can be used to, among other
///   things, create links to known paths and have them verified statically. Note that the
///   [`Display`] implementation for each field must return something that's compatible with its
///   [`Deserialize`] implementation.
///
/// Additionally the macro will verify the captures in the path matches the fields of the struct.
/// For example this fails to compile since the struct doesn't have a `team_id` field:
///
/// ```compile_fail
/// use serde::Deserialize;
/// use axum_extra::routing::TypedPath;
///
/// #[derive(TypedPath, Deserialize)]
/// #[typed_path("/users/{id}/teams/{team_id}")]
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
/// #[typed_path("/users/{id}")]
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
/// #[typed_path("/users/{id}")]
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
/// #[typed_path("/users/{id}", rejection(UsersMemberRejection))]
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
/// [`TypedMethod`]: super::TypedMethod
/// [`Path`]: axum::extract::Path
/// [`Display`]: std::fmt::Display
/// [`Deserialize`]: serde::Deserialize
/// [`PathRejection`]: axum::extract::rejection::PathRejection
pub trait TypedPath: std::fmt::Display {
    /// The path with optional captures such as `/users/{id}`.
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
    /// assert_eq!(path.to_uri(), "/users?page=1&per_page=10");
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

        let params_start = out.find('?').map(|i| i + 1).unwrap_or_else(|| {
            out.push('?');
            out.len()
        });
        let mut urlencoder = form_urlencoded::Serializer::for_suffix(&mut out, params_start);
        self.params
            .serialize(serde_html_form::ser::Serializer::new(&mut urlencoder))
            .unwrap_or_else(|err| {
                panic!(
                    "failed to URL encode value of type `{}`: {err}",
                    type_name::<T>(),
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
/// If you see it in type errors it's most likely because the first argument to your handler does
/// not implement [`TypedPath`] or does not match the type inferred by the routing method.
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
    use crate::routing::{
        Connect, Delete, Get, Head, Options, Patch, Post, Put, Query, RouterExt, Trace,
        TypedMethod, TypedPath,
    };
    use crate::test_helpers::TestClient;
    use axum::{http::StatusCode, routing::MethodFilter, Router};
    use serde::{Deserialize, Serialize};

    #[derive(TypedPath, Deserialize)]
    #[typed_path("/users/{id}")]
    struct UsersShow {
        id: i32,
    }

    impl TypedMethod for UsersShow {
        const METHOD: MethodFilter = MethodFilter::GET;
    }

    struct MethodOnly;

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

        assert_eq!(uri, "/users/1?foo=foo&bar=123&baz=true");
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

        assert_eq!(uri, "/users/1?foo=foo&bar=123&baz=true&qux=1337");
    }

    #[test]
    fn with_params_question_mark_no_params() {
        #[derive(TypedPath)]
        #[typed_path("/test?")]
        struct EndsWithQuestionMark;

        assert_eq!(EndsWithQuestionMark.to_uri(), "/test?");

        let path = EndsWithQuestionMark.with_query_params(Params {
            foo: "foo",
            bar: 123,
            baz: true,
        });

        let uri = path.to_uri();

        assert_eq!(uri, "/test?foo=foo&bar=123&baz=true");
    }

    #[test]
    fn method_wrappers_forward_path_operations() {
        let path = UsersShow { id: 1 };
        let get = Get(path);

        assert_eq!(get.to_string(), "/users/1");
        assert_eq!(get.to_uri(), "/users/1");
        assert_eq!(get.as_ref().id, 1);
        assert_eq!(get.into_inner().id, 1);

        assert_eq!(Get::<UsersShow>::PATH, UsersShow::PATH);
        assert_eq!(Get::<UsersShow>::METHOD, MethodFilter::GET);
        assert_eq!(Get::<MethodOnly>::METHOD, MethodFilter::GET);
        assert_eq!(Delete::<UsersShow>::METHOD, MethodFilter::DELETE);
        assert_eq!(Head::<UsersShow>::METHOD, MethodFilter::HEAD);
        assert_eq!(Options::<UsersShow>::METHOD, MethodFilter::OPTIONS);
        assert_eq!(Patch::<UsersShow>::METHOD, MethodFilter::PATCH);
        assert_eq!(Post::<UsersShow>::METHOD, MethodFilter::POST);
        assert_eq!(Put::<UsersShow>::METHOD, MethodFilter::PUT);
        assert_eq!(Query::<UsersShow>::METHOD, MethodFilter::QUERY);
        assert_eq!(Trace::<UsersShow>::METHOD, MethodFilter::TRACE);
        assert_eq!(Connect::<UsersShow>::METHOD, MethodFilter::CONNECT);
    }

    #[derive(TypedPath, TypedMethod, Deserialize)]
    #[typed_path("/typed/{id}")]
    #[typed_method(MethodFilter::GET)]
    struct TypedUser {
        id: u32,
    }

    async fn typed_user(TypedUser { id }: TypedUser) -> String {
        id.to_string()
    }

    async fn typed_user_with_explicit_method(TypedUser { id }: TypedUser) -> String {
        id.to_string()
    }

    #[tokio::test]
    async fn typed_method_and_path_are_composable() {
        let app = Router::new().typed(typed_user);
        let response = TestClient::new(app).get("/typed/42").await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await, "42");

        // A colocated type remains usable with the method-specific helpers.
        let _: Router = Router::new().typed_get(typed_user_with_explicit_method);
    }

    #[derive(TypedPath, Deserialize)]
    #[typed_path("/shared/{id}")]
    struct SharedUser {
        id: u32,
    }

    async fn get_shared(Get(SharedUser { id }): Get<SharedUser>) -> String {
        id.to_string()
    }

    async fn put_shared(Put(SharedUser { id }): Put<SharedUser>) -> String {
        id.to_string()
    }

    #[tokio::test]
    async fn method_wrappers_route_shared_paths() {
        let app = Router::new().typed(get_shared).typed(put_shared);
        let client = TestClient::new(app);

        let response = client.get("/shared/7").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await, "7");

        let response = client.put("/shared/8").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await, "8");
    }

    #[cfg(feature = "with-rejection")]
    #[allow(dead_code)] // just needs to compile
    fn supports_with_rejection() {
        use crate::routing::RouterExt;
        use axum::{
            extract::rejection::PathRejection,
            response::{IntoResponse, Response},
            Router,
        };
        async fn handler(_: crate::extract::WithRejection<UsersShow, MyRejection>) {}
        async fn typed_handler(_: crate::extract::WithRejection<UsersShow, MyRejection>) {}

        struct MyRejection {}

        impl IntoResponse for MyRejection {
            fn into_response(self) -> Response {
                unimplemented!()
            }
        }

        impl From<PathRejection> for MyRejection {
            fn from(_: PathRejection) -> Self {
                unimplemented!()
            }
        }

        let _: Router = Router::new().typed_get(handler);
        let _: Router = Router::new().typed(typed_handler);
    }
}
