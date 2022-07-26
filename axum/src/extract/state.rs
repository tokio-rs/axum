use async_trait::async_trait;
use axum_core::extract::{FromRequest, RequestParts};
use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};

/// Extractor for state.
///
/// # Examples
///
/// ## With `Router`
///
/// ```
/// use axum::{Router, routing::get, extract::State};
///
/// // the application state
/// //
/// // here you can put configuration, database connection pools, or whatever
/// // state you need
/// #[derive(Clone)]
/// struct AppState {}
///
/// let state = AppState {};
///
/// // create a `Router` that holds our state
/// let app = Router::with_state(state).route("/", get(handler));
///
/// async fn handler(
///     // access the state via the `State` extractor
///     // extracting a state of the wrong type results in a compile error
///     State(state): State<AppState>,
/// ) {
///     // use `state`...
/// }
/// # let _: Router<AppState> = app;
/// ```
///
/// ### Substates
///
/// [`State`] only allows a single state type but you can use [`From`] to extract "substates":
///
/// ```
/// use axum::{Router, routing::get, extract::State};
///
/// // the application state
/// #[derive(Clone)]
/// struct AppState {
///     // that holds some api specific state
///     api_state: ApiState,
/// }
///
/// // the api specific state
/// #[derive(Clone)]
/// struct ApiState {}
///
/// // support converting an `AppState` in an `ApiState`
/// impl From<AppState> for ApiState {
///     fn from(app_state: AppState) -> ApiState {
///         app_state.api_state
///     }
/// }
///
/// let state = AppState {
///     api_state: ApiState {},
/// };
///
/// let app = Router::with_state(state)
///     .route("/", get(handler))
///     .route("/api/users", get(api_users));
///
/// async fn api_users(
///     // access the api specific state
///     State(api_state): State<ApiState>,
/// ) {
/// }
///
/// async fn handler(
///     // we can still access to top level state
///     State(state): State<AppState>,
/// ) {
/// }
/// # let _: Router<AppState> = app;
/// ```
///
/// ## With `MethodRouter`
///
/// ```
/// use axum::{routing::get, extract::State};
///
/// #[derive(Clone)]
/// struct AppState {}
///
/// let state = AppState {};
///
/// let app = get(handler)
///     // provide the state so the handler can access it
///     .with_state(state);
///
/// async fn handler(State(state): State<AppState>) {
///     // use `state`...
/// }
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(app.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// ## With `Handler`
///
/// ```
/// use axum::{routing::get, handler::Handler, extract::State};
///
/// #[derive(Clone)]
/// struct AppState {}
///
/// let state = AppState {};
///
/// let app = handler.with_state(state);
///
/// async fn handler(State(state): State<AppState>) {
///     // use `state`...
/// }
///
/// # async {
/// axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
///     .serve(app.into_make_service())
///     .await
///     .expect("server failed");
/// # };
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct State<S>(pub S);

#[async_trait]
impl<B, OuterState, InnerState> FromRequest<B, OuterState> for State<InnerState>
where
    B: Send,
    OuterState: Clone + Into<InnerState> + Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<B, OuterState>) -> Result<Self, Self::Rejection> {
        let outer_state = req.state().clone();
        let inner_state = outer_state.into();
        Ok(Self(inner_state))
    }
}

impl<S> Deref for State<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> DerefMut for State<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
