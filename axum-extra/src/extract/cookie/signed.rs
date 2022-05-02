use super::{cookies_from_request, set_cookies};
use axum::{
    async_trait,
    extract::{FromRequest, RequestParts},
    response::{IntoResponse, IntoResponseParts, Response, ResponseParts},
    Extension,
};
use cookie_lib::SignedJar;
use cookie_lib::{Cookie, Key};
use std::{convert::Infallible, fmt, marker::PhantomData};

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
///             Redirect::to("/me"),
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
        let key = Extension::<K>::from_request(req).await?.0.into();

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

impl<K> IntoResponseParts for SignedCookieJar<K> {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        set_cookies(self.jar, res.headers_mut());
        Ok(res)
    }
}

impl<K> IntoResponse for SignedCookieJar<K> {
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
