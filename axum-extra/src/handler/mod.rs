//! Additional handler utilities.

use axum::{
    extract::{FromRequest, RequestParts},
    handler::Handler,
    response::{IntoResponse, Response},
};
use futures_util::future::{BoxFuture, FutureExt, Map};
use std::{future::Future, marker::PhantomData};

mod or;

pub use self::or::Or;

/// Trait for async functions that can be used to handle requests.
///
/// This trait is similar to [`Handler`] but rather than taking the request it takes the extracted
/// inputs.
///
/// The drawbacks of this trait is that you cannot apply middleware to individual handlers like you
/// can with [`Handler::layer`].
pub trait HandlerCallWithExtractors<T, B>: Sized {
    /// The type of future calling this handler returns.
    type Future: Future<Output = Response> + Send + 'static;

    /// Call the handler with the extracted inputs.
    fn call(self, extractors: T) -> <Self as HandlerCallWithExtractors<T, B>>::Future;

    /// Conver this `HandlerCallWithExtractors` into [`Handler`].
    fn into_handler(self) -> IntoHandler<Self, T, B> {
        IntoHandler {
            handler: self,
            _marker: PhantomData,
        }
    }

    /// Chain two handlers together, running the second one if the first one rejects.
    ///
    /// Note that this only moves to the next handler if an extractor fails. The response from
    /// handlers are not considered.
    ///
    /// # Example
    ///
    /// ```
    /// use axum_extra::handler::HandlerCallWithExtractors;
    /// use axum::{
    ///     Router,
    ///     async_trait,
    ///     routing::get,
    ///     extract::FromRequest,
    /// };
    ///
    /// // handlers for varying levels of access
    /// async fn admin(admin: AdminPermissions) {
    ///     // request came from an admin
    /// }
    ///
    /// async fn user(user: User) {
    ///     // we have a `User`
    /// }
    ///
    /// async fn guest() {
    ///     // `AdminPermissions` and `User` failed, so we're just a guest
    /// }
    ///
    /// // extractors for checking permissions
    /// struct AdminPermissions {}
    ///
    /// #[async_trait]
    /// impl<B: Send> FromRequest<B> for AdminPermissions {
    ///     // check for admin permissions...
    ///     # type Rejection = ();
    ///     # async fn from_request(req: &mut axum::extract::RequestParts<B>) -> Result<Self, Self::Rejection> {
    ///     #     todo!()
    ///     # }
    /// }
    ///
    /// struct User {}
    ///
    /// #[async_trait]
    /// impl<B: Send> FromRequest<B> for User {
    ///     // check for a logged in user...
    ///     # type Rejection = ();
    ///     # async fn from_request(req: &mut axum::extract::RequestParts<B>) -> Result<Self, Self::Rejection> {
    ///     #     todo!()
    ///     # }
    /// }
    ///
    /// let app = Router::new().route(
    ///     "/users/:id",
    ///     get(
    ///         // first try `admin`, if that rejects run `user`, finally falling back
    ///         // to `guest`
    ///         admin.or(user).or(guest)
    ///     )
    /// );
    /// # let _: Router = app;
    /// ```
    fn or<R, Rt>(self, rhs: R) -> Or<Self, R, T, Rt, B>
    where
        R: HandlerCallWithExtractors<Rt, B>,
    {
        Or {
            lhs: self,
            rhs,
            _marker: PhantomData,
        }
    }
}

macro_rules! impl_handler_call_with {
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<F, Fut, B, $($ty,)*> HandlerCallWithExtractors<($($ty,)*), B> for F
        where
            F: FnOnce($($ty,)*) -> Fut,
            Fut: Future + Send + 'static,
            Fut::Output: IntoResponse,
        {
            // this puts `futures_util` in our public API but thats fine in axum-extra
            type Future = Map<Fut, fn(Fut::Output) -> Response>;

            fn call(
                self,
                ($($ty,)*): ($($ty,)*),
            ) -> <Self as HandlerCallWithExtractors<($($ty,)*), B>>::Future {
                self($($ty,)*).map(IntoResponse::into_response)
            }
        }
    };
}

impl_handler_call_with!();
impl_handler_call_with!(T1);
impl_handler_call_with!(T1, T2);
impl_handler_call_with!(T1, T2, T3);
impl_handler_call_with!(T1, T2, T3, T4);
impl_handler_call_with!(T1, T2, T3, T4, T5);
impl_handler_call_with!(T1, T2, T3, T4, T5, T6);
impl_handler_call_with!(T1, T2, T3, T4, T5, T6, T7);
impl_handler_call_with!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_handler_call_with!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_handler_call_with!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_handler_call_with!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_handler_call_with!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_handler_call_with!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_handler_call_with!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_handler_call_with!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
impl_handler_call_with!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

/// A [`Handler`] created from a [`HandlerCallWithExtractors`].
///
/// Created with [`HandlerCallWithExtractors::into_handler`].
#[allow(missing_debug_implementations)]
pub struct IntoHandler<H, T, B> {
    handler: H,
    _marker: PhantomData<fn() -> (T, B)>,
}

impl<H, T, B> Handler<T, B> for IntoHandler<H, T, B>
where
    H: HandlerCallWithExtractors<T, B> + Clone + Send + 'static,
    T: FromRequest<B> + Send + 'static,
    T::Rejection: Send,
    B: Send + 'static,
{
    type Future = BoxFuture<'static, Response>;

    fn call(self, req: http::Request<B>) -> Self::Future {
        Box::pin(async move {
            let mut req = RequestParts::new(req);
            match req.extract::<T>().await {
                Ok(t) => self.handler.call(t).await,
                Err(rejection) => rejection.into_response(),
            }
        })
    }
}

impl<H, T, B> Copy for IntoHandler<H, T, B> where H: Copy {}

impl<H, T, B> Clone for IntoHandler<H, T, B>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            _marker: self._marker,
        }
    }
}
