//! Cookie parsing and cookie jar management.
//!
//! See [`CookieJar`] and [`SignedCookieJar`] for more details.

use axum::{
    async_trait,
    extract::{FromRequest, RequestParts},
    response::{IntoResponse, IntoResponseParts, Response, ResponseParts},
    Extension,
};
use cookie_lib::SignedJar;
use http::{
    header::{COOKIE, SET_COOKIE},
    HeaderMap,
};
use std::{convert::Infallible, fmt, marker::PhantomData};

pub use cookie_lib::{Cookie, Key};

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
/// ) -> impl IntoResponse {
///     if let Some(session_id) = authorize_and_create_session(auth.token()).await {
///         Ok((
///             // the updated jar must be returned for the changes
///             // to be included in the response
///             jar.add(Cookie::new("session_id", session_id)),
///             Redirect::to("/me".parse().unwrap()),
///         ))
///     } else {
///         Err(StatusCode::UNAUTHORIZED)
///     }
/// }
///
/// async fn me(jar: CookieJar) -> impl IntoResponse {
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
/// # let app: Router<axum::body::Body> = app;
/// ```
#[derive(Debug)]
pub struct CookieJar {
    jar: cookie_lib::CookieJar,
}

#[async_trait]
impl<B> FromRequest<B> for CookieJar
where
    B: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let mut jar = cookie_lib::CookieJar::new();
        for cookie in cookies_from_request(req) {
            jar.add_original(cookie);
        }
        Ok(Self { jar })
    }
}

fn cookies_from_request<B>(
    req: &mut RequestParts<B>,
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
    /// async fn handle(jar: CookieJar) -> impl IntoResponse {
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
    /// async fn handle(jar: CookieJar) -> impl IntoResponse {
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

/// Extractor that grabs signed cookies from the request and manages the jar.
///
/// All cookies will be signed and verified with a [`Key`]. Do not use this to store private data
/// as the values are still transmitted in plaintext.
///
/// Note that methods like [`SignedCookieJar::add`], [`SignedCookieJar::remove`], etc updates the
/// [`SignedCookieJar`] and returns it. This value _must_ be returned from the handler as part of
/// the response for the changes to be propagated.
///
/// # Example
///
/// ```rust
/// use axum::{
///     Router,
///     Extension,
///     routing::{post, get},
///     extract::TypedHeader,
///     response::{IntoResponse, Redirect},
///     headers::authorization::{Authorization, Bearer},
///     http::StatusCode,
/// };
/// use axum_extra::extract::cookie::{SignedCookieJar, Cookie, Key};
///
/// async fn create_session(
///     TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
///     jar: SignedCookieJar,
/// ) -> impl IntoResponse {
///     if let Some(session_id) = authorize_and_create_session(auth.token()).await {
///         Ok((
///             // the updated jar must be returned for the changes
///             // to be included in the response
///             jar.add(Cookie::new("session_id", session_id)),
///             Redirect::to("/me".parse().unwrap()),
///         ))
///     } else {
///         Err(StatusCode::UNAUTHORIZED)
///     }
/// }
///
/// async fn me(jar: SignedCookieJar) -> impl IntoResponse {
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
/// // Generate a secure key
/// //
/// // You probably don't wanna generate a new one each time the app starts though
/// let key = Key::generate();
///
/// let app = Router::new()
///     .route("/sessions", post(create_session))
///     .route("/me", get(me))
///     // add extension with the key so `SignedCookieJar` can access it
///     .layer(Extension(key));
/// # let app: Router<axum::body::Body> = app;
/// ```
pub struct SignedCookieJar<K = Key> {
    jar: cookie_lib::CookieJar,
    key: Key,
    // The key used to extract the key extension. Allows users to use multiple keys for different
    // jars. Maybe a library wants its own key.
    _marker: PhantomData<K>,
}

impl<K> fmt::Debug for SignedCookieJar<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignedCookieJar")
            .field("jar", &self.jar)
            .field("key", &"REDACTED")
            .finish()
    }
}

#[async_trait]
impl<B, K> FromRequest<B> for SignedCookieJar<K>
where
    B: Send,
    K: Into<Key> + Clone + Send + Sync + 'static,
{
    type Rejection = <axum::Extension<K> as FromRequest<B>>::Rejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Extension(key) = Extension::from_request(req).await?;

        let mut jar = cookie_lib::CookieJar::new();
        let mut signed_jar = jar.signed_mut(&key);
        for cookie in cookies_from_request(req) {
            if let Some(cookie) = signed_jar.verify(cookie) {
                signed_jar.add_original(cookie);
            }
        }

        Ok(Self {
            jar,
            key,
            _marker: PhantomData,
        })
    }
}

impl<K> SignedCookieJar<K> {
    /// Get a cookie from the jar.
    ///
    /// If the cookie exists and its authenticity and integrity can be verified then it is returned
    /// in plaintext.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum_extra::extract::cookie::SignedCookieJar;
    /// use axum::response::IntoResponse;
    ///
    /// async fn handle(jar: SignedCookieJar) {
    ///     let value: Option<String> = jar
    ///         .get("foo")
    ///         .map(|cookie| cookie.value().to_owned());
    /// }
    /// ```
    pub fn get(&self, name: &str) -> Option<Cookie<'static>> {
        self.signed_jar().get(name)
    }

    /// Remove a cookie from the jar.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum_extra::extract::cookie::{SignedCookieJar, Cookie};
    /// use axum::response::IntoResponse;
    ///
    /// async fn handle(jar: SignedCookieJar) -> impl IntoResponse {
    ///     jar.remove(Cookie::named("foo"))
    /// }
    /// ```
    #[must_use]
    pub fn remove(mut self, cookie: Cookie<'static>) -> Self {
        self.signed_jar_mut().remove(cookie);
        self
    }

    /// Add a cookie to the jar.
    ///
    /// The value will automatically be percent-encoded.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum_extra::extract::cookie::{SignedCookieJar, Cookie};
    /// use axum::response::IntoResponse;
    ///
    /// async fn handle(jar: SignedCookieJar) -> impl IntoResponse {
    ///     jar.add(Cookie::new("foo", "bar"))
    /// }
    /// ```
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, cookie: Cookie<'static>) -> Self {
        self.signed_jar_mut().add(cookie);
        self
    }

    /// Verifies the authenticity and integrity of `cookie`, returning the plaintext version if
    /// verification succeeds or `None` otherwise.
    pub fn verify(&self, cookie: Cookie<'static>) -> Option<Cookie<'static>> {
        self.signed_jar().verify(cookie)
    }

    /// Get an iterator over all cookies in the jar.
    ///
    /// Only cookies with valid authenticity and integrity are yielded by the iterator.
    pub fn iter(&self) -> impl Iterator<Item = Cookie<'static>> + '_ {
        SignedCookieJarIter {
            jar: self,
            iter: self.jar.iter(),
        }
    }

    fn signed_jar(&self) -> SignedJar<&'_ cookie_lib::CookieJar> {
        self.jar.signed(&self.key)
    }

    fn signed_jar_mut(&mut self) -> SignedJar<&'_ mut cookie_lib::CookieJar> {
        self.jar.signed_mut(&self.key)
    }
}

impl IntoResponseParts for SignedCookieJar {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        set_cookies(self.jar, res.headers_mut());
        Ok(res)
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

impl IntoResponse for SignedCookieJar {
    fn into_response(self) -> Response {
        (self, ()).into_response()
    }
}

struct SignedCookieJarIter<'a, K> {
    jar: &'a SignedCookieJar<K>,
    iter: cookie_lib::Iter<'a>,
}

impl<'a, K> Iterator for SignedCookieJarIter<'a, K> {
    type Item = Cookie<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let cookie = self.iter.next()?;

            if let Some(cookie) = self.jar.get(cookie.name()) {
                return Some(cookie);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
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

                let app = Router::<Body>::new()
                    .route("/set", get(set_cookie))
                    .route("/get", get(get_cookie))
                    .route("/remove", get(remove_cookie))
                    .layer(Extension(Key::generate()));

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

    #[tokio::test]
    async fn signed_cannot_access_invalid_cookies() {
        async fn get_cookie(jar: SignedCookieJar) -> impl IntoResponse {
            format!("{:?}", jar.get("key"))
        }

        let app = Router::<Body>::new()
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
