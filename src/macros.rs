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
                #[pin] pub(crate) future: $actual,
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
            fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
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

        #[allow(deprecated)]
        impl $crate::response::IntoResponse for $name {
            type Body = http_body::Full<bytes::Bytes>;
            type BodyError = std::convert::Infallible;

            fn into_response(self) -> http::Response<Self::Body> {
                let mut res = http::Response::new(http_body::Full::from($body));
                *res.status_mut() = http::StatusCode::$status;
                res
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", $body)
            }
        }

        impl std::error::Error for $name {}
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
                E: Into<tower::BoxError>,
            {
                Self(crate::Error::new(err))
            }
        }

        impl IntoResponse for $name {
            type Body = http_body::Full<bytes::Bytes>;
            type BodyError = std::convert::Infallible;

            fn into_response(self) -> http::Response<Self::Body> {
                let mut res =
                    http::Response::new(http_body::Full::from(format!(concat!($body, ": {}"), self.0)));
                *res.status_mut() = http::StatusCode::$status;
                res
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
                #[allow(missing_docs, deprecated)]
                $variant($variant)
            ),+
        }

        impl $crate::response::IntoResponse for $name {
            type Body = http_body::Full<bytes::Bytes>;
            type BodyError = std::convert::Infallible;

            fn into_response(self) -> http::Response<Self::Body> {
                match self {
                    $(
                        Self::$variant(inner) => inner.into_response(),
                    )+
                }
            }
        }

        $(
            #[allow(deprecated)]
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
