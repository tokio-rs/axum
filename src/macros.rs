//! Internal macros

macro_rules! opaque_future {
    ($(#[$m:meta])* pub type $name:ident = $actual:ty;) => {
        opaque_future! {
            $(#[$m])*
            pub type $name<> = $actual;
        }
    };

    ($(#[$m:meta])* pub type $name:ident<$($param:ident),*> = $actual:ty;) => {
        #[pin_project::pin_project]
        $(#[$m])*
        pub struct $name<$($param),*>(#[pin] pub(crate) $actual)
        where;

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
            fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
                self.project().0.poll(cx)
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
            fn into_response(self) -> http::Response<$crate::body::Body> {
                let mut res = http::Response::new($crate::body::Body::from($body));
                *res.status_mut() = http::StatusCode::$status;
                res
            }
        }
    };

    (
        #[status = $status:ident]
        #[body = $body:expr]
        $(#[$m:meta])*
        pub struct $name:ident (BoxError);
    ) => {
        $(#[$m])*
        #[derive(Debug)]
        pub struct $name(pub(super) tower::BoxError);

        impl $name {
            pub(super) fn from_err<E>(err: E) -> Self
            where
                E: Into<tower::BoxError>,
            {
                Self(err.into())
            }
        }

        impl IntoResponse for $name {
            fn into_response(self) -> http::Response<Body> {
                let mut res =
                    http::Response::new(Body::from(format!(concat!($body, ": {}"), self.0)));
                *res.status_mut() = http::StatusCode::$status;
                res
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
            fn into_response(self) -> http::Response<$crate::body::Body> {
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
    };
}
