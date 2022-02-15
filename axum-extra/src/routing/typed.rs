use super::sealed::Sealed;
use axum::{
    async_trait,
    extract::{FromRequest, RequestParts},
    handler::Handler,
    routing::MethodRouter,
};
use std::{convert::Infallible, fmt, marker::PhantomData};

/// A typed request path.
///
/// This is used to statically connect a path to its corresponding handler using
/// [`RouterExt::typed_route`]. See that method for more details.
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
/// - A [`FromRequest`] implementation compatible with [`RouterExt::typed_route`]. This
/// implementation uses [`Path`] and thus your struct must also implement [`serde::Deserialize`],
/// unless it's a unit struct.
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
/// [`FromRequest`]: axum::extract::FromRequest
/// [`RouterExt::typed_route`]: super::RouterExt::typed_route
/// [`Path`]: axum::extract::Path
/// [`Display`]: std::fmt::Display
/// [`Deserialize`]: serde::Deserialize
pub trait TypedPath: std::fmt::Display {
    /// The path with optional captures such as `/users/:id`.
    const PATH: &'static str;
}

/// A typed HTTP method.
///
/// This is used to statically connect an HTTP method to its corresponding handler using
/// [`RouterExt::typed_route`]. See that method for more details.
///
/// This trait is sealed such that it cannot be implemented outside this crate.
///
/// [`RouterExt::typed_route`]: super::RouterExt::typed_route
pub trait TypedMethod: Sealed {
    /// Wrap a handler in a [`MethodRouter`] that accepts this type's corresponding HTTP method.
    fn apply_method_router<H, B, T>(handler: H) -> MethodRouter<B>
    where
        H: Handler<T, B>,
        B: Send + 'static,
        T: 'static;

    /// Check if the request matches this type's corresponding HTTP method.
    fn matches_method(method: &http::Method) -> bool;
}

macro_rules! typed_method {
    ($name:ident, $method_router_constructor:ident, $method:ident) => {
        #[doc = concat!("A `TypedMethod` that accepts `", stringify!($method), "` requests.")]
        #[derive(Clone, Copy, Debug)]
        pub struct $name;

        impl Sealed for $name {}

        impl TypedMethod for $name {
            fn apply_method_router<H, B, T>(handler: H) -> MethodRouter<B>
            where
                H: Handler<T, B>,
                B: Send + 'static,
                T: 'static,
            {
                axum::routing::$method_router_constructor(handler)
            }

            fn matches_method(method: &http::Method) -> bool {
                method == http::Method::$method
            }
        }

        #[async_trait]
        impl<B> FromRequest<B> for $name
        where
            B: Send,
        {
            type Rejection = http::StatusCode;

            async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
                if Self::matches_method(req.method()) {
                    Ok(Self)
                } else {
                    Err(http::StatusCode::NOT_FOUND)
                }
            }
        }
    };
}

typed_method!(Delete, delete, DELETE);
typed_method!(Get, get, GET);
typed_method!(Head, head, HEAD);
typed_method!(Options, options, OPTIONS);
typed_method!(Patch, patch, PATCH);
typed_method!(Post, post, POST);
typed_method!(Put, put, PUT);
typed_method!(Trace, trace, TRACE);

/// A [`TypedMethod`] that accepts all HTTP methods.
///
/// # Example
///
/// ```rust
/// use axum_extra::routing::{TypedPath, Any, RouterExt};
/// use axum::Router;
///
/// #[derive(TypedPath)]
/// #[typed_path("/foo")]
/// struct Foo;
///
/// // This accepts `/foo` with any HTTP method.
/// async fn foo(_: Any, _: Foo) {}
///
/// let app = Router::new().typed_route(foo);
/// #
/// # let app: Router<axum::body::Body> = app;
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Any;

impl Sealed for Any {}

impl TypedMethod for Any {
    fn apply_method_router<H, B, T>(handler: H) -> MethodRouter<B>
    where
        H: Handler<T, B>,
        B: Send + 'static,
        T: 'static,
    {
        axum::routing::any(handler)
    }

    fn matches_method(_method: &http::Method) -> bool {
        true
    }
}

#[async_trait]
impl<B> FromRequest<B> for Any
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(_: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        Ok(Self)
    }
}

/// A [`TypedMethod`] that accepts one of a number of HTTP methods.
///
/// # Example
///
/// ```rust
/// use axum_extra::routing::{TypedPath, OneOf, Patch, Put, RouterExt};
/// use axum::Router;
///
/// #[derive(TypedPath)]
/// #[typed_path("/foo")]
/// struct Foo;
///
/// // This accepts `PATCH /foo` and `PUT /foo`
/// async fn foo(_: OneOf<(Patch, Put)>, _: Foo) {}
///
/// let app = Router::new().typed_route(foo);
/// #
/// # let app: Router<axum::body::Body> = app;
/// ```
pub struct OneOf<T>(PhantomData<T>);

macro_rules! one_of {
    ($($ty:ident),* $(,)?) => {
        impl<$($ty,)*> TypedMethod for OneOf<($($ty,)*)>
        where
            $( $ty: TypedMethod, )*
        {
            #[allow(clippy::redundant_clone, unused_mut, unused_variables)]
            fn apply_method_router<H, B, T>(handler: H) -> MethodRouter<B>
            where
                H: Handler<T, B>,
                B: Send + 'static,
                T: 'static,
            {
                let mut method_router = MethodRouter::new();
                $(
                    method_router = method_router.merge($ty::apply_method_router(handler.clone()));
                )*
                method_router
            }

            #[allow(unused_variables)]
            fn matches_method(method: &http::Method) -> bool {
                $(
                    if $ty::matches_method(method) {
                        return true;
                    }
                )*
                false
            }
        }

        impl<$($ty,)*> Sealed for OneOf<($($ty,)*)> {}

        #[async_trait]
        impl<B, $($ty,)*> FromRequest<B> for OneOf<($($ty,)*)>
        where
            B: Send,
            $( $ty: TypedMethod + FromRequest<B>, )*
        {
            type Rejection = http::StatusCode;

            async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
                if Self::matches_method(req.method()) {
                    Ok(Self(PhantomData))
                } else {
                    Err(http::StatusCode::NOT_FOUND)
                }
            }
        }
    };
}

one_of!();
one_of!(T1,);
one_of!(T1, T2);
one_of!(T1, T2, T3);
one_of!(T1, T2, T3, T4);
one_of!(T1, T2, T3, T4, T5);
one_of!(T1, T2, T3, T4, T5, T6);
one_of!(T1, T2, T3, T4, T5, T6, T7);
one_of!(T1, T2, T3, T4, T5, T6, T7, T8);

impl<T> fmt::Debug for OneOf<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OneOf")
            .field(&format_args!("{}", std::any::type_name::<T>()))
            .finish()
    }
}

impl<T> Default for OneOf<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> Clone for OneOf<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<T> Copy for OneOf<T> {}

/// Utility trait used with [`RouterExt`] to ensure the first element of a tuple type is a
/// given type.
///
/// If you see it in type errors its most likely because the first argument to your handler doesn't
/// implement [`TypedPath`].
///
/// You normally shouldn't have to use this trait directly.
///
/// It is sealed such that it cannot be implemented outside this crate.
///
/// [`RouterExt`]: super::RouterExt
pub trait FirstTwoElementsAre<M, P>: Sealed {}

macro_rules! impl_first_element_is {
    ( $($ty:ident),* $(,)? ) => {
        impl<M, P, $($ty,)*> FirstTwoElementsAre<M, P> for (M, P, $($ty,)*)
        where
            M: TypedMethod,
            P: TypedPath,
        {}

        impl<M, P, $($ty,)*> Sealed for (M, P, $($ty,)*)
        where
            M: TypedMethod,
            P: TypedPath,
        {}
    };
}

impl_first_element_is!();
impl_first_element_is!(T1);
impl_first_element_is!(T1, T2);
impl_first_element_is!(T1, T2, T3);
impl_first_element_is!(T1, T2, T3, T4);
impl_first_element_is!(T1, T2, T3, T4, T5);
impl_first_element_is!(T1, T2, T3, T4, T5, T6);
impl_first_element_is!(T1, T2, T3, T4, T5, T6, T7);
impl_first_element_is!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_first_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_first_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_first_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_first_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_first_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_first_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_first_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
impl_first_element_is!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);
