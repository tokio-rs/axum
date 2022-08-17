use async_trait::async_trait;
use axum_core::extract::{FromRequest, FromRef, RequestParts};
use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};

/// Extractor for state.
///
/// Note this extractor is not available to middleware. See ["Accessing state in
/// middleware"][state-from-middleware] for how to access state in middleware.
///
/// [state-from-middleware]: ../middleware/index.html#accessing-state-in-middleware
///
/// # With `Router`
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
/// # With `MethodRouter`
///
/// ```
/// use axum::{routing::get, extract::State};
///
/// #[derive(Clone)]
/// struct AppState {}
///
/// let state = AppState {};
///
/// let method_router_with_state = get(handler)
///     // provide the state so the handler can access it
///     .with_state(state);
///
/// async fn handler(State(state): State<AppState>) {
///     // use `state`...
/// }
/// # async {
/// # axum::Server::bind(&"".parse().unwrap()).serve(method_router_with_state.into_make_service()).await.unwrap();
/// # };
/// ```
///
/// # With `Handler`
///
/// ```
/// use axum::{routing::get, handler::Handler, extract::State};
///
/// #[derive(Clone)]
/// struct AppState {}
///
/// let state = AppState {};
///
/// async fn handler(State(state): State<AppState>) {
///     // use `state`...
/// }
///
/// // provide the state so the handler can access it
/// let handler_with_state = handler.with_state(state);
///
/// # async {
/// axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
///     .serve(handler_with_state.into_make_service())
///     .await
///     .expect("server failed");
/// # };
/// ```
///
/// # Substates
///
/// [`State`] only allows a single state type but you can use [`From`] to extract "substates":
///
/// ```
/// use axum::{Router, routing::get, extract::{State, FromRef}};
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
/// impl FromRef<AppState> for ApiState {
///     fn from_ref(app_state: &AppState) -> ApiState {
///         app_state.api_state.clone()
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
/// # For library authors
///
/// If you're writing a library that has an extractor that needs state, this is the recommended way
/// to do it:
///
/// ```rust
/// use axum_core::extract::{FromRequest, RequestParts, FromRef};
/// use async_trait::async_trait;
/// use std::convert::Infallible;
///
/// // the extractor your library provides
/// struct MyLibraryExtractor;
///
/// #[async_trait]
/// impl<S, B> FromRequest<S, B> for MyLibraryExtractor
/// where
///     B: Send,
///     // keep `S` generic but require that it can produce a `MyLibraryState`
///     // this means users will have to implement `FromRef<UserState> for MyLibraryState`
///     MyLibraryState: FromRef<S>,
///     S: Send,
/// {
///     type Rejection = Infallible;
///
///     async fn from_request(req: &mut RequestParts<S, B>) -> Result<Self, Self::Rejection> {
///         // get a `MyLibraryState` from a reference to the state
///         let state = MyLibraryState::from_ref(req.state());
///
///         // ...
///         # todo!()
///     }
/// }
///
/// // the state your library needs
/// struct MyLibraryState {
///     // ...
/// }
/// ```
///
/// Note that you don't need to use the `State` extractor since you can access the state directly
/// from [`RequestParts`].
#[derive(Debug, Default, Clone, Copy)]
pub struct State<S>(pub S);

#[async_trait]
impl<B, OuterState, InnerState> FromRequest<OuterState, B> for State<InnerState>
where
    B: Send,
    InnerState: FromRef<OuterState>,
    OuterState: Send,
{
    type Rejection = Infallible;

    async fn from_request(req: &mut RequestParts<OuterState, B>) -> Result<Self, Self::Rejection> {
        let inner_state = InnerState::from_ref(req.state());
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
