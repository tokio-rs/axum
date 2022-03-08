use axum::extract::{Extension, Form, Path};
use axum::response::{Html, Redirect};
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};
use tower_cookies::Cookies;

use crate::{Cookie, IntoResponse, PgPool, Uri};

// Struct is needed for transferring to Tera where will be used later.
#[derive(Serialize)]
pub struct Todo {
    id: i32,
    title: String,
    description: String,
    checked: bool,
    login: String,
}

pub async fn list_todos(
    Extension(pool): Extension<PgPool>,
    Extension(tera): Extension<Tera>,
    cookies: Cookies,
) -> Html<String> {
    // Check if exists a cookie with the given key. At this case key is `login`.
    match cookies.get("login") {
        Some(login) => {
            let is_doesnt_exist = sqlx::query!(
                "SELECT * FROM accounts WHERE login = $1",
                login.value().to_string()
            )
                .fetch_one(&pool)
                .await;

            // If the value of this cookie is empty or doesn't exist account with the given `login` we return Login Page Template.
            if login.value().to_string().is_empty() || is_doesnt_exist.is_err() {
                return Html(tera.render("login.html", &Context::new()).unwrap());
            }
        }
        None => {
            // We add given cookies that don't exist.
            cookies.add(Cookie::new("login", String::new()));
            cookies.add(Cookie::new("password", String::new()));

            return Html(tera.render("login.html", &Context::new()).unwrap());
        }
    }

    let account = cookies.get("login").unwrap().value().to_string();
    // We're getting all Todos of the given account.
    let todos = sqlx::query_as!(Todo, "SELECT * FROM todos WHERE login = $1", account)
        .fetch_all(&pool)
        .await
        .unwrap();

    // And after succeed getting Todos we transfer them ot Tera's `Context`.
    let mut context = Context::new();
    context.insert("todos", &todos); // Todos as a vector of `Todo` that derive macro `Serialize`.
    context.insert("account", &account); // also we transfer an account's `login`.

    Html(tera.render("index.html", &context).unwrap())
}

pub async fn get_description(
    Path(id): Path<u32>,
    Extension(pool): Extension<PgPool>,
    Extension(tera): Extension<Tera>,
) -> Html<String> {
    // Getting Description of Todo by an ID that we got from the URL: /<id>.
    let todo = sqlx::query!("SELECT * FROM todos WHERE id = $1", id as i32)
        .fetch_one(&pool)
        .await
        .unwrap();

    let mut context = Context::new();
    context.insert("title", &todo.title);
    context.insert("description", &todo.description);
    context.insert("id", &todo.id);
    context.insert(
        "checked",
        &if todo.checked { "Done" } else { "Not yet Done" },
    );

    Html(tera.render("description.html", &context).unwrap())
}

pub async fn delete_all_done_todos(
    cookies: Cookies,
    Extension(pool): Extension<PgPool>,
) -> impl IntoResponse {
    // Deleting all DONE Todos of the associated account.
    sqlx::query!(
        "DELETE FROM todos WHERE checked = true AND login = $1",
        cookies.get("login").unwrap().value().to_string()
    )
        .execute(&pool)
        .await
        .unwrap();

    Redirect::to(Uri::from_static("/"))
}

pub async fn delete_all_todos(
    cookies: Cookies,
    Extension(pool): Extension<PgPool>,
) -> impl IntoResponse {
    // Deleting ALL Todos of the associated account.
    sqlx::query!(
        "DELETE FROM todos WHERE login = $1",
        cookies.get("login").unwrap().value().to_string()
    )
        .execute(&pool)
        .await
        .unwrap();

    Redirect::to(Uri::from_static("/"))
}

pub async fn delete_todo(
    Path(id): Path<u32>,
    Extension(pool): Extension<PgPool>,
) -> impl IntoResponse {
    // Deleting Todo by an ID.
    sqlx::query!("DELETE FROM todos WHERE id = $1", id as i32)
        .execute(&pool)
        .await
        .unwrap();

    Redirect::to(Uri::from_static("/"))
}

// Struct is needed for the Deserialization of the form.
#[derive(Deserialize)]
pub struct NewTodo {
    title: String,
    description: String,
}

// When we do a GET Request by URL: /new.
// We get an HTML with the form.
pub async fn editing_new_todo<'a>() -> Html<&'a str> {
    Html(include_str!("../templates/new.html"))
}

// After we do a POST Request by URL: /new, where we add new Todo for the given account.
pub async fn create_todo(
    Form(todo): Form<NewTodo>,
    cookies: Cookies,
    Extension(pool): Extension<PgPool>,
) -> impl IntoResponse {
    sqlx::query!(
        "INSERT INTO todos(title, description, login) VALUES($1, $2, $3)",
        todo.title,
        todo.description,
        cookies.get("login").unwrap().value().to_string()
    )
        .execute(&pool)
        .await
        .unwrap();

    Redirect::to(Uri::from_static("/"))
}

#[derive(Deserialize)]
pub struct UpdatedTodo {
    title: String,
    description: String,
    checked: Option<String>,
}

// We return an HTML of Editing Todo with the content of this Todo.
pub async fn edit_todo(
    Path(id): Path<u32>,
    Extension(pool): Extension<PgPool>,
    Extension(tera): Extension<Tera>,
) -> Html<String> {
    let todo = sqlx::query_as!(Todo, "SELECT * FROM todos WHERE id = $1", id as i32)
        .fetch_one(&pool)
        .await
        .unwrap();

    let mut context = Context::new();
    context.insert("todo", &todo);

    Html(tera.render("edit.html", &context).unwrap())
}

// After we did changes with Todo we make a POST Request.
pub async fn update_todo(
    Path(id): Path<u32>,
    Form(new_content): Form<UpdatedTodo>,
    Extension(pool): Extension<PgPool>,
) -> impl IntoResponse {
    sqlx::query!(
        "UPDATE todos SET title = $1, description = $2, checked = $3 WHERE id = $4",
        new_content.title,
        new_content.description,
        new_content.checked.is_some(),
        id as i32,
    )
        .execute(&pool)
        .await
        .unwrap();

    Redirect::to(Uri::from_static("/"))
}
