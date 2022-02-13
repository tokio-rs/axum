/// Type safe routing.
///
/// # Example
///
/// ```rust
/// use serde::Deserialize;
/// use axum_macros::TypedPath;
/// use axum::{Router, extract::Json};
/// use axum_extra::routing::{
///     typed,
///     RouterExt, // for `Router::with`
/// };
///
/// // A type safe route with `/users/:id` as its associated path.
/// #[derive(Deserialize, TypedPath)]
/// #[typed_path("/users/:id")]
/// struct UsersMember {
///     id: u32,
/// }
///
/// // A regular handler function that takes `UsersMember` as the first argument
/// // and thus creates a typed connection between this handler and the `/users/:id` route.
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
///     .with(typed::get(users_show))
///     // Add multiple handlers for `/users` depending on the HTTP method.
///     .with(typed::post(users_create).delete(users_destroy))
///     // We can still add regular routes.
///     .route("/foo", get(|| async { /* ... */ }));
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
///     Json(payload): Json<Payload>,
/// ) {
///     // ...
/// }
///
/// async fn users_destroy(_: UsersCollection) { /* ... */ }
///
/// #
/// # let app: Router<axum::body::Body> = app;
/// ```
use super::sealed::Sealed;

pub trait TypedPath: std::fmt::Display {
    const PATH: &'static str;
}

/// Utility trait used with [`TypedRouter`] to ensure the first element of a tuple type is a
/// given type.
///
/// If you see it in type errors its most likely because the first argument to your handler doesn't
/// implement [`TypedPath`].
///
/// You normally shouldn't have to use this trait directly.
///
/// It is sealed such that it cannot be implemented outside this crate.
pub trait FirstElementIs<P>: Sealed {}

macro_rules! impl_first_element_is {
    ( $($ty:ident),* $(,)? ) => {
        impl<P, $($ty,)*> FirstElementIs<P> for (P, $($ty,)*) {}

        impl<P, $($ty,)*> Sealed for (P, $($ty,)*) {}
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
