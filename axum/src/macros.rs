//! Internal macros

macro_rules! opaque_future {
    ($(#[$m:meta])* pub type $name:ident = $actual:ty;) => {
        opaque_future! {
            $(#[$m])*
            pub type $name<> = $actual;
        }
    };

    ($(#[$m:meta])* pub type $name:ident<$($param:ident),*> = $actual:ty;) => {
        pin_project_lite::pin_project! {
            $(#[$m])*
            pub struct $name<$($param),*> {
                #[pin] future: $actual,
            }
        }

        impl<$($param),*> $name<$($param),*> {
            pub(crate) fn new(future: $actual) -> Self {
                Self { future }
            }
        }

        impl<$($param),*> std::fmt::Debug for $name<$($param),*> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple(stringify!($name)).field(&format_args!("...")).finish()
            }
        }

        impl<$($param),*> std::future::Future for $name<$($param),*>
        where
            $actual: std::future::Future,
        {
            type Output = <$actual as std::future::Future>::Output;

            #[inline]
            fn poll(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Self::Output> {
                self.project().future.poll(cx)
            }
        }
    };
}

macro_rules! define_rejection {
    (
        #[status = $status:ident]
        #[body = $body:expr]
        $(#[$m:meta])*
        pub struct $name:ident;
    ) => {
        $(#[$m])*
        #[derive(Debug)]
        #[non_exhaustive]
        pub struct $name;

        impl $crate::response::IntoResponse for $name {
            fn into_response(self) -> $crate::response::Response {
                (http::StatusCode::$status, $body).into_response()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", $body)
            }
        }

        impl std::error::Error for $name {}

        impl Default for $name {
            fn default() -> Self {
                Self
            }
        }
    };

    (
        #[status = $status:ident]
        #[body = $body:expr]
        $(#[$m:meta])*
        pub struct $name:ident (Error);
    ) => {
        $(#[$m])*
        #[derive(Debug)]
        pub struct $name(pub(crate) crate::Error);

        impl $name {
            pub(crate) fn from_err<E>(err: E) -> Self
            where
                E: Into<crate::BoxError>,
            {
                Self(crate::Error::new(err))
            }
        }

        impl crate::response::IntoResponse for $name {
            fn into_response(self) -> $crate::response::Response {
                (
                    http::StatusCode::$status,
                    format!(concat!($body, ": {}"), self.0),
                ).into_response()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", $body)
            }
        }

        impl std::error::Error for $name {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }
    };
}

macro_rules! composite_rejection {
    (
        $(#[$m:meta])*
        pub enum $name:ident {
            $($variant:ident),+
            $(,)?
        }
    ) => {
        $(#[$m])*
        #[derive(Debug)]
        #[non_exhaustive]
        pub enum $name {
            $(
                #[allow(missing_docs)]
                $variant($variant)
            ),+
        }

        impl $crate::response::IntoResponse for $name {
            fn into_response(self) -> $crate::response::Response {
                match self {
                    $(
                        Self::$variant(inner) => inner.into_response(),
                    )+
                }
            }
        }

        $(
            impl From<$variant> for $name {
                fn from(inner: $variant) -> Self {
                    Self::$variant(inner)
                }
            }
        )+

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        Self::$variant(inner) => write!(f, "{}", inner),
                    )+
                }
            }
        }

        impl std::error::Error for $name {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self {
                    $(
                        Self::$variant(inner) => Some(inner),
                    )+
                }
            }
        }
    };
}

macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!(T1);
        $name!(T1, T2);
        $name!(T1, T2, T3);
        $name!(T1, T2, T3, T4);
        $name!(T1, T2, T3, T4, T5);
        $name!(T1, T2, T3, T4, T5, T6);
        $name!(T1, T2, T3, T4, T5, T6, T7);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);
    };
}
