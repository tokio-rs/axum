//! Run with
//!
//! ```not_rust
//! cargo run -p example-dependency-injection
//! ```

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let user_repo = InMemoryUserRepo::default();

    // Two separate decisions go into injecting a dependency.
    //
    // A. Whether to abstract over it with a trait at all.
    //
    //    A trait (`dyn SomeTrait` or generic `T: SomeTrait`) is only worth it when you have more
    //    than one implementation, for example a real database-backed repo in production and an
    //    in-memory one in tests, or when a lower layer must not depend on the concrete type. With
    //    a single implementation and no such boundary, use the concrete struct instead (see
    //    `using_leaked`). A trait there only costs you extra bounds, heap indirection for `dyn`,
    //    and async methods that are not yet `dyn`-compatible without a crate like `async-trait`.
    //
    //    1. Using trait objects (`dyn SomeTrait`)
    //        - Pros
    //            - Likely leads to simpler code due to fewer type parameters.
    //        - Cons
    //            - Less flexible because we can only use `dyn`-compatible ("object safe") traits.
    //            - Small amount of additional runtime overhead due to dynamic dispatch.
    //              This is likely to be negligible.
    //    2. Using generics (`T where T: SomeTrait`)
    //        - Pros
    //            - More flexible since all traits can be used.
    //            - No dynamic dispatch. Calls are static and can be inlined. This says nothing
    //              about the cost of sharing the value, which is decision B.
    //        - Cons
    //            - Additional type parameters and trait bounds can lead to more complex code and
    //              boilerplate.
    //
    //    Using trait objects is recommended unless you really need generics.
    //
    // B. How the value is shared across handlers, whichever choice you made in A.
    //    axum clones the state on every request that extracts it, so this comes down to what a
    //    clone costs.
    //
    //    1. Behind an `Arc`. Either an explicit `Arc<dyn SomeTrait>` or `Arc<ConcreteType>`, or
    //       an `Arc` that already lives inside a cheaply cloned concrete type. `InMemoryUserRepo`
    //       is the latter, which is how `using_generic` shares its data.
    //        - Pros
    //            - Works for values that come and go during the program, not just ones that live
    //              for the whole process.
    //        - Cons
    //            - Every clone bumps an atomic reference count. Small, but not free.
    //    2. Leaked to a `&'static` reference with `Box::leak`.
    //        - Pros
    //            - Nothing to reference count. A `&'static` reference is just a pointer.
    //            - `&'static T` (and `&'static dyn Trait`) is `Copy`, so the state never needs a
    //              real clone. That helps when you move it into a `tokio::spawn`, a closure, or
    //              another thread, the way `create_user_leaked` does below. Moving it to another
    //              thread also needs the value to be `Sync`. `Copy` alone covers same-thread
    //              closures.
    //        - Cons
    //            - The value is never dropped, so it leaks. Fine for things built once that live
    //              for the whole program, which is the usual case for a server's dependencies,
    //              and wrong for anything created and thrown away repeatedly.

    let using_dyn = Router::new()
        .route("/users/{id}", get(get_user_dyn))
        .route("/users", post(create_user_dyn))
        .with_state(AppStateDyn {
            user_repo: Arc::new(user_repo.clone()),
        });

    // `AppStateGeneric<InMemoryUserRepo>` carries the repo by value. Its data still sits behind
    // the repo's own `Arc<Mutex<..>>`, so cloning the state per request only bumps that inner
    // count, the same cost as the `Arc<dyn UserRepo>` above.
    let using_generic = Router::new()
        .route("/users/{id}", get(get_user_generic::<InMemoryUserRepo>))
        .route("/users", post(create_user_generic::<InMemoryUserRepo>))
        .with_state(AppStateGeneric {
            user_repo: user_repo.clone(),
        });

    // Only one `UserRepo` exists, so we skip the trait object and leak the concrete
    // `InMemoryUserRepo`. Leaking would work through a trait just as well:
    // `Box::leak(Box::new(user_repo) as Box<dyn UserRepo>)` hands back a `&'static dyn UserRepo`.
    // `Box::leak` takes an owned `Box<T>` and returns a `&'static mut T`, which coerces to the
    // shared `&'static T` the state stores. That box is never freed, which is the price of a
    // `Copy` reference instead of an `Arc`.
    let using_leaked = Router::new()
        .route("/users/{id}", get(get_user_leaked))
        .route("/users", post(create_user_leaked))
        .with_state(AppStateLeaked {
            user_repo: Box::leak(Box::new(user_repo)),
        });

    let app = Router::new()
        .nest("/dyn", using_dyn)
        .nest("/generic", using_generic)
        .nest("/leaked", using_leaked);

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await;
}

#[derive(Clone)]
struct AppStateDyn {
    user_repo: Arc<dyn UserRepo>,
}

#[derive(Clone)]
struct AppStateGeneric<T> {
    user_repo: T,
}

// `&'static InMemoryUserRepo` is `Copy`, which makes the whole state `Copy`. Handlers and spawned
// tasks can take it by value without an `Arc` or a clone. It holds the concrete type rather than
// `dyn UserRepo`, since leaking has nothing to do with whether you use a trait.
#[derive(Clone, Copy)]
struct AppStateLeaked {
    user_repo: &'static InMemoryUserRepo,
}

#[derive(Debug, Serialize, Clone)]
struct User {
    id: Uuid,
    name: String,
}

#[derive(Deserialize)]
struct UserParams {
    name: String,
}

async fn create_user_dyn(
    State(state): State<AppStateDyn>,
    Json(params): Json<UserParams>,
) -> Json<User> {
    let user = User {
        id: Uuid::new_v4(),
        name: params.name,
    };

    state.user_repo.save_user(&user);

    Json(user)
}

async fn get_user_dyn(
    State(state): State<AppStateDyn>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, StatusCode> {
    match state.user_repo.get_user(id) {
        Some(user) => Ok(Json(user)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn create_user_generic<T>(
    State(state): State<AppStateGeneric<T>>,
    Json(params): Json<UserParams>,
) -> Json<User>
where
    T: UserRepo,
{
    let user = User {
        id: Uuid::new_v4(),
        name: params.name,
    };

    state.user_repo.save_user(&user);

    Json(user)
}

async fn get_user_generic<T>(
    State(state): State<AppStateGeneric<T>>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, StatusCode>
where
    T: UserRepo,
{
    match state.user_repo.get_user(id) {
        Some(user) => Ok(Json(user)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn create_user_leaked(
    State(state): State<AppStateLeaked>,
    Json(params): Json<UserParams>,
) -> Json<User> {
    let user = User {
        id: Uuid::new_v4(),
        name: params.name,
    };

    let user_repo = state.user_repo;
    user_repo.save_user(&user);

    // `user_repo` is a `Copy` `&'static` reference, so kicking off fire-and-forget work that
    // needs it just means moving that reference into the task, with no `Arc::clone`. An
    // `Arc`-based state would have to clone the `Arc` before the `move`.
    let id = user.id;
    tokio::spawn(async move {
        if let Some(user) = user_repo.get_user(id) {
            tracing::debug!("background follow-up for freshly created user {}", user.name);
        }
    });

    Json(user)
}

async fn get_user_leaked(
    State(state): State<AppStateLeaked>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, StatusCode> {
    match state.user_repo.get_user(id) {
        Some(user) => Ok(Json(user)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

trait UserRepo: Send + Sync {
    fn get_user(&self, id: Uuid) -> Option<User>;

    fn save_user(&self, user: &User);
}

#[derive(Debug, Clone, Default)]
struct InMemoryUserRepo {
    map: Arc<Mutex<HashMap<Uuid, User>>>,
}

impl UserRepo for InMemoryUserRepo {
    fn get_user(&self, id: Uuid) -> Option<User> {
        self.map.lock().unwrap().get(&id).cloned()
    }

    fn save_user(&self, user: &User) {
        self.map.lock().unwrap().insert(user.id, user.clone());
    }
}
