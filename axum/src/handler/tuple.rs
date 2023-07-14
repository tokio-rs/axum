use std::future::Future;

use pin_project_lite::pin_project;

use crate::response::Response;

use super::Handler;

macro_rules! impl_handler {
    (
        [$($ty:ident, $tyt:ident, $tyi:tt),*], $last:ident, $lastt:ident, $lasti:tt
    ) => {
        #[allow(non_snake_case)]
        mod $last {
            use super::*;

            pin_project! {
                #[project = AnyOfProj]
                pub enum AnyOf<$($ty,)* $last> {
                    $( $ty{ #[pin] pinned: $ty }, )*
                    $last{ #[pin] pinned: $last },
                }
            }

            impl<$($ty,)* $last> Future for AnyOf<$($ty,)* $last> where
                $( $ty: Future<Output = Response>, )*
                $last: Future<Output = Response>,
            {
                type Output = Response;

                fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
                    match self.project() {
                        $( AnyOfProj::$ty { pinned } => pinned.poll(cx), )*
                        AnyOfProj::$last { pinned } => pinned.poll(cx),
                    }
                }
            }

            impl<S, $($ty, $tyt,)* $last, $lastt> Handler<($($tyt,)* $lastt,), S> for ($($ty,)* $last,) where
            $($ty: Handler<$tyt, S>, )*
            $last: Handler<$lastt, S>,
            S: Send + Sync + 'static,
            {
                type Future = AnyOf<$($ty::Future,)* $last::Future>;

                fn call(self, req: axum_core::extract::Request, state: S) -> Self::Future {
                    $(
                        if self.$tyi.can_accept(&req, &state) {
                            return AnyOf::$ty{ pinned: self.$tyi.call(req, state) }
                        }
                    )*
                    AnyOf::$last{ pinned: self.$lasti.call(req, state) }
                }
            }
        }
    };
}

impl_handler!([], T1, T1T, 0);
impl_handler!([T1, T1T, 0], T2, T2T, 1);
impl_handler!([T1, T1T, 0, T2, T2T, 1], T3, T3T, 2);
impl_handler!([T1, T1T, 0, T2, T2T, 1, T3, T3T, 2], T4, T4T, 3);
