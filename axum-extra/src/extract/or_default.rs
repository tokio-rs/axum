use axum::{
    async_trait,
    extract::{FromRequest, RequestParts},
};
use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};

/// Use an extractor's default value if it fails.
///
/// `OrDefault` wraps another extractors, that implements [`Default`], and uses its default value
/// if the inner extractor fails.
///
/// # Example
///
/// Serializing query params for pagination and using default values for missing or invalid params:
///
/// ```rust
/// use axum_extra::extract::OrDefault;
/// use axum::extract::Query;
/// use serde::Deserialize;
///
/// #[derive(Default, Deserialize)]
/// struct Pagination {
///     #[serde(default)]
///     page: Page,
///     #[serde(default)]
///     per_page: PerPage,
/// }
///
/// #[derive(Deserialize, Copy, Clone)]
/// #[serde(transparent)]
/// struct Page(usize);
///
/// impl Default for Page {
///     fn default() -> Self {
///         Self(1)
///     }
/// }
///
/// #[derive(Deserialize, Copy, Clone)]
/// #[serde(transparent)]
/// struct PerPage(usize);
///
/// impl Default for PerPage {
///     fn default() -> Self {
///         Self(30)
///     }
/// }
///
/// async fn handler(
///     pagination: OrDefault<Query<Pagination>>,
/// ) {
///     // the default value for `pagination`, `pagination.page`
///     // and `pagination.per_page` will be used if the query params
///     // are missing or invalid
/// }
/// ```
///
/// Note that for this particular example if the query params contains invalid values (like
/// `?page=invalid&per_page=10`), `OrDefault` will catch the error and return
/// `Pagination::default`. Thus `per_page` would be `30` instead of `10` like it was in the query
/// string.
#[derive(Copy, Clone, Debug, Default)]
pub struct OrDefault<T>(pub T);

#[async_trait]
impl<B, T> FromRequest<B> for OrDefault<T>
where
    B: Send,
    T: FromRequest<B> + Default,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let value = T::from_request(req).await.unwrap_or_default();
        Ok(Self(value))
    }
}

impl<T> Deref for OrDefault<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for OrDefault<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use axum::extract::Query;
    use http::Request;
    use serde::Deserialize;

    #[tokio::test]
    async fn test_something() {
        #[derive(Default, Deserialize, Debug, PartialEq, Eq)]
        #[allow(dead_code)]
        struct Pagination {
            #[serde(default)]
            page: Page,
            #[serde(default)]
            per_page: PerPage,
        }

        #[derive(Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
        #[serde(transparent)]
        struct Page(usize);

        impl Default for Page {
            fn default() -> Self {
                Self(1)
            }
        }

        #[derive(Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
        #[serde(transparent)]
        struct PerPage(usize);

        impl Default for PerPage {
            fn default() -> Self {
                Self(30)
            }
        }

        async fn test(uri: &str, expected: Pagination) {
            let mut req = RequestParts::new(Request::builder().uri(uri).body(()).unwrap());
            let OrDefault(Query(pagination)) =
                OrDefault::<Query<Pagination>>::from_request(&mut req)
                    .await
                    .unwrap();
            assert_eq!(pagination, expected);
        }

        test(
            "/?page=2&per_page=10",
            Pagination {
                page: Page(2),
                per_page: PerPage(10),
            },
        )
        .await;

        test(
            "/?page=2",
            Pagination {
                page: Page(2),
                per_page: PerPage::default(),
            },
        )
        .await;

        test(
            "/?per_page=30",
            Pagination {
                page: Page::default(),
                per_page: PerPage(30),
            },
        )
        .await;

        test(
            "/",
            Pagination {
                page: Page::default(),
                per_page: PerPage::default(),
            },
        )
        .await;

        test(
            "/?page=invalid&per_page=10",
            Pagination {
                page: Page::default(),
                per_page: PerPage::default(),
            },
        )
        .await;
    }
}
