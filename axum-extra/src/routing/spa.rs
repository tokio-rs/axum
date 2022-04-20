use axum::{
    body::{Body, HttpBody},
    error_handling::HandleError,
    response::Response,
    routing::{get_service, Route},
    Router,
};
use http::{Request, StatusCode};
use std::{
    any::type_name,
    convert::Infallible,
    fmt,
    future::{ready, Ready},
    io,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Arc,
};
use tower_http::services::{ServeDir, ServeFile};
use tower_service::Service;

/// Router for single page applications.
///
/// `SpaRouter` gives a routing setup commonly used for single page applications.
///
/// # Example
///
/// ```
/// use axum_extra::routing::SpaRouter;
/// use axum::{Router, routing::get};
///
/// let spa = SpaRouter::new("/assets", "dist");
///
/// let app = Router::new()
///     // `SpaRouter` implements `Into<Router>` so it works with `merge`
///     .merge(spa)
///     // we can still add other routes
///     .route("/api/foo", get(api_foo));
/// # let _: Router<axum::body::Body> = app;
///
/// async fn api_foo() {}
/// ```
///
/// With this setup we get this behavior:
///
/// - `GET /` will serve `index.html`
/// - `GET /assets/app.js` will serve `dist/app.js` assuming that file exists
/// - `GET /assets/doesnt_exist` will respond with `404 Not Found` assuming no
///   such file exists
/// - `GET /some/other/path` will serve `index.html` since there isn't another
///   route for it
/// - `GET /api/foo` will serve the `api_foo` handler function
///
/// # Serving assets from the root
///
/// Assets can also be served from the root, i.e. not nested `/assets` or similar:
///
/// ```
/// use axum_extra::routing::SpaRouter;
/// use axum::{Router, routing::get};
///
/// let spa = SpaRouter::new("/", "dist");
///
/// let app = Router::new().merge(spa);
/// # let _: Router<axum::body::Body> = app;
/// ```
pub struct SpaRouter<B = Body, T = (), F = fn(io::Error) -> Ready<StatusCode>> {
    paths: Arc<Paths>,
    handle_error: F,
    serve_index_file_on_missing_asset: bool,
    _marker: PhantomData<fn() -> (B, T)>,
}

#[derive(Debug)]
struct Paths {
    assets_path: String,
    assets_dir: PathBuf,
    index_file: PathBuf,
}

impl<B> SpaRouter<B, (), fn(io::Error) -> Ready<StatusCode>> {
    /// Create a new `SpaRouter`.
    ///
    /// Assets will be served at `GET /{serve_assets_at}` from the directory at `assets_dir`.
    ///
    /// The index file defaults to `assets_dir.join("index.html")`.
    pub fn new<P>(serve_assets_at: &str, assets_dir: P) -> Self
    where
        P: AsRef<Path>,
    {
        let path = assets_dir.as_ref();
        Self {
            paths: Arc::new(Paths {
                assets_path: serve_assets_at.to_owned(),
                assets_dir: path.to_owned(),
                index_file: path.join("index.html"),
            }),
            handle_error: |_| ready(StatusCode::INTERNAL_SERVER_ERROR),
            serve_index_file_on_missing_asset: serve_assets_at == "/",
            _marker: PhantomData,
        }
    }
}

impl<B, T, F> SpaRouter<B, T, F> {
    /// Set the path to the index file.
    ///
    /// `path` must be relative to `assets_dir` passed to [`SpaRouter::new`].
    ///
    /// # Example
    ///
    /// ```
    /// use axum_extra::routing::SpaRouter;
    /// use axum::Router;
    ///
    /// let spa = SpaRouter::new("/assets", "dist")
    ///     .index_file("another_file.html");
    ///
    /// let app = Router::new().merge(spa);
    /// # let _: Router<axum::body::Body> = app;
    /// ```
    pub fn index_file<P>(mut self, path: P) -> Self
    where
        P: AsRef<Path>,
    {
        self.paths = Arc::new(Paths {
            assets_path: self.paths.assets_path.clone(),
            assets_dir: self.paths.assets_dir.clone(),
            index_file: self.paths.assets_dir.join(path),
        });
        self
    }

    /// Change the function used to handle unknown IO errors.
    ///
    /// `SpaRouter` automatically maps missing files and permission denied to
    /// `404 Not Found`. The callback given here will be used for other IO errors.
    ///
    /// See [`axum::error_handling::HandleErrorLayer`] for more details.
    ///
    /// # Example
    ///
    /// ```
    /// use std::io;
    /// use axum_extra::routing::SpaRouter;
    /// use axum::{Router, http::{Method, Uri}};
    ///
    /// let spa = SpaRouter::new("/assets", "dist").handle_error(handle_error);
    ///
    /// async fn handle_error(method: Method, uri: Uri, err: io::Error) -> String {
    ///     format!("{} {} failed with {}", method, uri, err)
    /// }
    ///
    /// let app = Router::new().merge(spa);
    /// # let _: Router<axum::body::Body> = app;
    /// ```
    pub fn handle_error<T2, F2>(self, f: F2) -> SpaRouter<B, T2, F2> {
        SpaRouter {
            paths: self.paths,
            handle_error: f,
            serve_index_file_on_missing_asset: self.serve_index_file_on_missing_asset,
            _marker: PhantomData,
        }
    }

    /// Change whats served when an asset is not found.
    ///
    /// By default if an asset is not found `SpaRouter` will respond with `404 Not Found`. Calling
    /// this method with `true` changes that such that the index file is served instead. The status
    /// code will be `200 OK`
    ///
    /// # Example
    ///
    /// ```
    /// use axum_extra::routing::SpaRouter;
    /// use axum::Router;
    ///
    /// let spa = SpaRouter::new("/assets", "dist")
    ///     // if `/assets/something-that-doesnt-exist` is called the index file will be sent back
    ///     .serve_index_file_on_missing_asset(true);
    ///
    /// let app = Router::new().merge(spa);
    /// # let _: Router<axum::body::Body> = app;
    /// ```
    ///
    /// # When serving assets at `/`
    ///
    /// If you're serving assets that `/` then you don't need to call
    /// `.serve_index_file_on_missing_asset(true)` as that is done automatically.
    ///
    /// ```
    /// use axum_extra::routing::SpaRouter;
    ///
    /// let spa = SpaRouter::new("/", "dist");
    ///
    /// // we don't need to call `.serve_index_file_on_missing_asset(true)`
    /// ```
    pub fn serve_index_file_on_missing_asset(mut self, flag: bool) -> Self {
        self.serve_index_file_on_missing_asset = flag;
        self
    }
}

impl<B, F, T> From<SpaRouter<B, T, F>> for Router<B>
where
    F: Clone + Send + 'static,
    HandleError<Route<B, io::Error>, F, T>:
        Service<Request<B>, Response = Response, Error = Infallible>,
    <HandleError<Route<B, io::Error>, F, T> as Service<Request<B>>>::Future: Send,
    B: HttpBody + Default + Send + 'static,
    T: 'static,
{
    #[allow(warnings)]
    fn from(spa: SpaRouter<B, T, F>) -> Self {
        use axum::{handler::Handler, response::IntoResponse};
        use tower::ServiceExt;

        let mut serve_index = get_service(ServeFile::new(&spa.paths.index_file))
            .handle_error(spa.handle_error.clone());

        let serve_asset_or_fallback_to_index = {
            let serve_index_file_on_missing_asset = spa.serve_index_file_on_missing_asset;
            let mut index_svc = serve_index.clone();

            let mut assets_svc = get_service(
                ServeDir::new(&spa.paths.assets_dir).append_index_html_on_directories(false),
            )
            .handle_error(spa.handle_error.clone());

            move |req: Request<B>| async move {
                let req_clone = clone_request_without_body(&req);

                let assets_res = assets_svc.call(req).await.into_response();

                if serve_index_file_on_missing_asset && assets_res.status() == StatusCode::NOT_FOUND
                {
                    index_svc.call(req_clone).await.into_response()
                } else {
                    assets_res
                }
            }
        };

        if spa.paths.assets_path == "/" {
            Router::new().fallback(serve_asset_or_fallback_to_index.into_service())
        } else {
            Router::new()
                .nest(
                    &spa.paths.assets_path,
                    serve_asset_or_fallback_to_index.into_service(),
                )
                .fallback(serve_index)
        }
    }
}

fn clone_request_without_body<B>(req: &Request<B>) -> Request<B>
where
    B: Default,
{
    let mut req_clone = Request::new(B::default());
    *req_clone.method_mut() = req.method().clone();
    *req_clone.uri_mut() = req.uri().clone();
    *req_clone.headers_mut() = req.headers().clone();
    req_clone
}

impl<B, T, F> fmt::Debug for SpaRouter<B, T, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            paths,
            handle_error: _,
            serve_index_file_on_missing_asset,
            _marker,
        } = self;

        f.debug_struct("SpaRouter")
            .field("paths", &paths)
            .field("handle_error", &format_args!("{}", type_name::<F>()))
            .field("request_body_type", &format_args!("{}", type_name::<B>()))
            .field(
                "extractor_input_type",
                &format_args!("{}", type_name::<T>()),
            )
            .field(
                "serve_index_file_on_missing_asset",
                &serve_index_file_on_missing_asset,
            )
            .finish()
    }
}

impl<B, T, F> Clone for SpaRouter<B, T, F>
where
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            paths: self.paths.clone(),
            handle_error: self.handle_error.clone(),
            serve_index_file_on_missing_asset: self.serve_index_file_on_missing_asset,
            _marker: self._marker,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use axum::{
        http::{Method, Uri},
        routing::get,
    };

    #[tokio::test]
    async fn serve_assets_at_root_and_serve_index_on_not_found() {
        // serve_index_file_on_missing_asset is automatically set to `true` when serving assets at `/`,
        // otherwise all requests would 404
        //
        // therefore testing the `false` variant doesn't make much sense

        let app = Router::new()
            .route("/foo", get(|| async { "GET /foo" }))
            .merge(SpaRouter::new("/", "test_files"));
        let client = TestClient::new(app);

        // route that exists
        let res = client.get("/foo").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "GET /foo");

        // route that doesn't exist
        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<h1>Hello, World!</h1>\n");

        // asset that exists
        let res = client.get("/script.js").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "console.log('hi')\n");

        // asset that doesn't exist
        let res = client.get("/doesnt_exist").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<h1>Hello, World!</h1>\n");
    }

    #[tokio::test]
    async fn serve_assets_at_path_root_and_serve_index_on_not_found() {
        let app = Router::new()
            .route("/foo", get(|| async { "GET /foo" }))
            .merge(SpaRouter::new("/assets", "test_files").serve_index_file_on_missing_asset(true));
        let client = TestClient::new(app);

        // route that exists
        let res = client.get("/foo").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "GET /foo");

        // route that doesn't exist
        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<h1>Hello, World!</h1>\n");

        // asset that exists
        let res = client.get("/assets/script.js").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "console.log('hi')\n");

        // asset that doesn't exist
        let res = client.get("/assets/doesnt_exist").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<h1>Hello, World!</h1>\n");
    }

    #[tokio::test]
    async fn serve_assets_at_path_root_and_dont_serve_index_on_not_found() {
        let app = Router::new()
            .route("/foo", get(|| async { "GET /foo" }))
            .merge(
                SpaRouter::new("/assets", "test_files").serve_index_file_on_missing_asset(false),
            );
        let client = TestClient::new(app);

        // route that exists
        let res = client.get("/foo").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "GET /foo");

        // route that doesn't exist
        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<h1>Hello, World!</h1>\n");

        // asset that exists
        let res = client.get("/assets/script.js").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "console.log('hi')\n");

        // asset that doesn't exist
        let res = client.get("/assets/doesnt_exist").send().await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert_eq!(res.text().await, "");
    }

    #[tokio::test]
    async fn setting_index_file() {
        let app =
            Router::new().merge(SpaRouter::new("/assets", "test_files").index_file("index_2.html"));
        let client = TestClient::new(app);

        let res = client.get("/").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<strong>Hello, World!</strong>\n");

        let res = client.get("/some/random/path").send().await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await, "<strong>Hello, World!</strong>\n");
    }

    // this should just compile
    #[allow(dead_code)]
    fn setting_error_handler() {
        async fn handle_error(method: Method, uri: Uri, err: io::Error) -> (StatusCode, String) {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("{} {} failed. Error: {}", method, uri, err),
            )
        }

        let spa = SpaRouter::new("/assets", "test_files").handle_error(handle_error);

        Router::<Body>::new().merge(spa);
    }
}
