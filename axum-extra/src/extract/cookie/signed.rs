use super::{cookies_from_request, set_cookies};
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    response::{IntoResponse, IntoResponseParts, Response, ResponseParts},
};
use cookie::SignedJar;
use cookie::{Cookie, Key};
use http::{request::Parts, HeaderMap};
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
///     routing::{post, get},
///     extract::{TypedHeader, FromRef},
///     response::{IntoResponse, Redirect},
///     headers::authorization::{Authorization, Bearer},
///     http::StatusCode,
/// };
/// use axum_extra::extract::cookie::{SignedCookieJar, Cookie, Key};
///
/// async fn create_session(
///     TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
///     jar: SignedCookieJar,
/// ) -> Result<(SignedCookieJar, Redirect), StatusCode> {
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
/// async fn me(jar: SignedCookieJar) -> Result<(), StatusCode> {
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
/// // our application state
/// #[derive(Clone)]
/// struct AppState {
///     // that holds the key used to sign cookies
///     key: Key,
/// }
///
/// // this impl tells `SignedCookieJar` how to access the key from our state
/// impl FromRef<AppState> for Key {
///     fn from_ref(state: &AppState) -> Self {
///         state.key.clone()
///     }
/// }
///
/// let state = AppState {
///     // Generate a secure key
///     //
///     // You probably don't wanna generate a new one each time the app starts though
///     key: Key::generate(),
/// };
///
/// let app = Router::new()
///     .route("/sessions", post(create_session))
///     .route("/me", get(me))
///     .with_state(state);
/// # let _: axum::Router = app;
/// ```
/// If you have been using `Arc<AppState>` you cannot implement `FromRef<Arc<AppState>> for Key`.
/// You can use a new type instead:
///
/// ```rust
/// # use axum::extract::FromRef;
/// # use axum_extra::extract::cookie::{PrivateCookieJar, Cookie, Key};
/// use std::sync::Arc;
/// use std::ops::Deref;
///
/// #[derive(Clone)]
/// struct AppState(Arc<InnerState>);
///
/// // deref so you can still access the inner fields easily
/// impl Deref for AppState {
///     type Target = InnerState;
///
///     fn deref(&self) -> &Self::Target {
///         &*self.0
///     }
/// }
///
/// struct InnerState {
///     key: Key
/// }
///
/// impl FromRef<AppState> for Key {
///     fn from_ref(state: &AppState) -> Self {
///         state.0.key.clone()
///     }
/// }
/// ```
pub struct SignedCookieJar<K = Key> {
    jar: cookie::CookieJar,
    key: Key,
    // The key used to extract the key. Allows users to use multiple keys for different
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
impl<S, K> FromRequestParts<S> for SignedCookieJar<K>
where
    S: Send + Sync,
    K: FromRef<S> + Into<Key>,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let k = K::from_ref(state);
        let key = k.into();
        let SignedCookieJar {
            jar,
            key,
            _marker: _,
        } = SignedCookieJar::from_headers(&parts.headers, key);
        Ok(SignedCookieJar {
            jar,
            key,
            _marker: PhantomData,
        })
    }
}

impl SignedCookieJar {
    /// Create a new `SignedCookieJar` from a map of request headers.
    ///
    /// The valid cookies in `headers` will be added to the jar.
    ///
    /// This is intended to be used in middleware and other places where it might be difficult to
    /// run extractors. Normally you should create `SignedCookieJar`s through [`FromRequestParts`].
    ///
    /// [`FromRequestParts`]: axum::extract::FromRequestParts
    pub fn from_headers(headers: &HeaderMap, key: Key) -> Self {
        let mut jar = cookie::CookieJar::new();
        let mut signed_jar = jar.signed_mut(&key);
        for cookie in cookies_from_request(headers) {
            if let Some(cookie) = signed_jar.verify(cookie) {
                signed_jar.add_original(cookie);
            }
        }

        Self {
            jar,
            key,
            _marker: PhantomData,
        }
    }

    /// Create a new empty `SignedCookieJar`.
    ///
    /// This is intended to be used in middleware and other places where it might be difficult to
    /// run extractors. Normally you should create `SignedCookieJar`s through [`FromRequestParts`].
    ///
    /// [`FromRequestParts`]: axum::extract::FromRequestParts
    pub fn new(key: Key) -> Self {
        Self {
            jar: Default::default(),
            key,
            _marker: PhantomData,
        }
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
    /// async fn handle(jar: SignedCookieJar) -> SignedCookieJar {
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
    /// async fn handle(jar: SignedCookieJar) -> SignedCookieJar {
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

    fn signed_jar(&self) -> SignedJar<&'_ cookie::CookieJar> {
        self.jar.signed(&self.key)
    }

    fn signed_jar_mut(&mut self) -> SignedJar<&'_ mut cookie::CookieJar> {
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
    iter: cookie::Iter<'a>,
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

impl<K> Clone for SignedCookieJar<K> {
    fn clone(&self) -> Self {
        Self {
            jar: self.jar.clone(),
            key: self.key.clone(),
            _marker: self._marker,
        }
    }
}
