//! Cookie parsing and cookie jar management.
//!
//! See [`CookieJar`], [`SignedCookieJar`], and [`PrivateCookieJar`] for more details.

use axum::{
    async_trait,
    extract::{FromRequest, RequestParts},
    response::{IntoResponse, IntoResponseParts, Response, ResponseParts},
};
use http::{
    header::{COOKIE, SET_COOKIE},
    HeaderMap,
};
use std::convert::Infallible;

#[cfg(feature = "cookie-private")]
mod private;
#[cfg(feature = "cookie-signed")]
mod signed;

#[cfg(feature = "cookie-private")]
pub use self::private::PrivateCookieJar;
#[cfg(feature = "cookie-signed")]
pub use self::signed::SignedCookieJar;

pub use cookie_lib::{Cookie, Expiration, SameSite};

#[cfg(any(feature = "cookie-signed", feature = "cookie-private"))]
pub use cookie_lib::Key;

/// Extractor that grabs cookies from the request and manages the jar.
///
/// Note that methods like [`CookieJar::add`], [`CookieJar::remove`], etc updates the [`CookieJar`]
/// and returns it. This value _must_ be returned from the handler as part of the response for the
/// changes to be propagated.
///
/// # Example
///
/// ```rust
/// use axum::{
///     Router,
///     routing::{post, get},
///     extract::TypedHeader,
///     response::{IntoResponse, Redirect},
///     headers::authorization::{Authorization, Bearer},
///     http::StatusCode,
/// };
/// use axum_extra::extract::cookie::{CookieJar, Cookie};
///
/// async fn create_session(
///     TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
///     jar: CookieJar,
/// ) -> Result<(CookieJar, Redirect), StatusCode> {
///     if let Some(session_id) = authorize_and_create_session(auth.token()).await {
///         Ok((
///             // the updated jar must be returned for the changes
///             // to be included in the response
///             jar.add(Cookie::new("session_id", session_id)),
///             Redirect::to("/me"),
///         ))
///     } else {
///         Err(StatusCode::UNAUTHORIZED)
///     }
/// }
///
/// async fn me(jar: CookieJar) -> Result<(), StatusCode> {
///     if let Some(session_id) = jar.get("session_id") {
///         // fetch and render user...
///         # Ok(())
///     } else {
///         Err(StatusCode::UNAUTHORIZED)
///     }
/// }
///
/// async fn authorize_and_create_session(token: &str) -> Option<String> {
///     // authorize the user and create a session...
///     # todo!()
/// }
///
/// let app = Router::new()
///     .route("/sessions", post(create_session))
///     .route("/me", get(me));
/// # let app: Router = app;
/// ```
#[derive(Debug)]
pub struct CookieJar {
    jar: cookie_lib::CookieJar,
}

#[async_trait]
impl<B, S> FromRequest<B, S> for CookieJar
where
    B: Send,
    S: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B, S>) -> Result<Self, Self::Rejection> {
        let mut jar = cookie_lib::CookieJar::new();
        for cookie in cookies_from_request(req) {
            jar.add_original(cookie);
        }
        Ok(Self { jar })
    }
}

fn cookies_from_request<B, S>(
    req: &mut RequestParts<B, S>,
) -> impl Iterator<Item = Cookie<'static>> + '_ {
    req.headers()
        .get_all(COOKIE)
        .into_iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|cookie| Cookie::parse_encoded(cookie.to_owned()).ok())
}

impl CookieJar {
    /// Get a cookie from the jar.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum_extra::extract::cookie::CookieJar;
    /// use axum::response::IntoResponse;
    ///
    /// async fn handle(jar: CookieJar) {
    ///     let value: Option<String> = jar
    ///         .get("foo")
    ///         .map(|cookie| cookie.value().to_owned());
    /// }
    /// ```
    pub fn get(&self, name: &str) -> Option<&Cookie<'static>> {
        self.jar.get(name)
    }

    /// Remove a cookie from the jar.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum_extra::extract::cookie::{CookieJar, Cookie};
    /// use axum::response::IntoResponse;
    ///
    /// async fn handle(jar: CookieJar) -> CookieJar {
    ///     jar.remove(Cookie::named("foo"))
    /// }
    /// ```
    #[must_use]
    pub fn remove(mut self, cookie: Cookie<'static>) -> Self {
        self.jar.remove(cookie);
        self
    }

    /// Add a cookie to the jar.
    ///
    /// The value will automatically be percent-encoded.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum_extra::extract::cookie::{CookieJar, Cookie};
    /// use axum::response::IntoResponse;
    ///
    /// async fn handle(jar: CookieJar) -> CookieJar {
    ///     jar.add(Cookie::new("foo", "bar"))
    /// }
    /// ```
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, cookie: Cookie<'static>) -> Self {
        self.jar.add(cookie);
        self
    }

    /// Get an iterator over all cookies in the jar.
    pub fn iter(&self) -> impl Iterator<Item = &'_ Cookie<'static>> {
        self.jar.iter()
    }
}

impl IntoResponseParts for CookieJar {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        set_cookies(self.jar, res.headers_mut());
        Ok(res)
    }
}

impl IntoResponse for CookieJar {
    fn into_response(self) -> Response {
        (self, ()).into_response()
    }
}

fn set_cookies(jar: cookie_lib::CookieJar, headers: &mut HeaderMap) {
    for cookie in jar.delta() {
        if let Ok(header_value) = cookie.encoded().to_string().parse() {
            headers.append(SET_COOKIE, header_value);
        }
    }

    // we don't need to call `jar.reset_delta()` because `into_response_parts` consumes the cookie
    // jar so it cannot be called multiple times.
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Extension, Router};
    use tower::ServiceExt;

    macro_rules! cookie_test {
        ($name:ident, $jar:ty) => {
            #[tokio::test]
            async fn $name() {
                async fn set_cookie(jar: $jar) -> impl IntoResponse {
                    jar.add(Cookie::new("key", "value"))
                }

                async fn get_cookie(jar: $jar) -> impl IntoResponse {
                    jar.get("key").unwrap().value().to_owned()
                }

                async fn remove_cookie(jar: $jar) -> impl IntoResponse {
                    jar.remove(Cookie::named("key"))
                }

                let app = Router::<_, Body>::new()
                    .route("/set", get(set_cookie))
                    .route("/get", get(get_cookie))
                    .route("/remove", get(remove_cookie))
                    .layer(Extension(Key::generate()))
                    .layer(Extension(CustomKey(Key::generate())));

                let res = app
                    .clone()
                    .oneshot(Request::builder().uri("/set").body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                let cookie_value = res.headers()["set-cookie"].to_str().unwrap();

                let res = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/get")
                            .header("cookie", cookie_value)
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                let body = body_text(res).await;
                assert_eq!(body, "value");

                let res = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/remove")
                            .header("cookie", cookie_value)
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert!(res.headers()["set-cookie"]
                    .to_str()
                    .unwrap()
                    .contains("key=;"));
            }
        };
    }

    cookie_test!(plaintext_cookies, CookieJar);
    cookie_test!(signed_cookies, SignedCookieJar);
    cookie_test!(signed_cookies_with_custom_key, SignedCookieJar<CustomKey>);
    cookie_test!(private_cookies, PrivateCookieJar);
    cookie_test!(private_cookies_with_custom_key, PrivateCookieJar<CustomKey>);

    #[derive(Clone)]
    struct CustomKey(Key);

    impl From<CustomKey> for Key {
        fn from(custom: CustomKey) -> Self {
            custom.0
        }
    }

    #[tokio::test]
    async fn signed_cannot_access_invalid_cookies() {
        async fn get_cookie(jar: SignedCookieJar) -> impl IntoResponse {
            format!("{:?}", jar.get("key"))
        }

        let app = Router::<_, Body>::new()
            .route("/get", get(get_cookie))
            .layer(Extension(Key::generate()));

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/get")
                    .header("cookie", "key=value")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = body_text(res).await;
        assert_eq!(body, "None");
    }

    async fn body_text<B>(body: B) -> String
    where
        B: axum::body::HttpBody,
        B::Error: std::fmt::Debug,
    {
        let bytes = hyper::body::to_bytes(body).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }
}
