use askama::Template;
use axum::{prelude::*, response::IntoResponse};
use http::{Response, StatusCode};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // build our application with some routes
    let app = route("/greet/:name", get(greet));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    hyper::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn greet(params: extract::UrlParamsMap) -> impl IntoResponse {
    let name = params
        .get("name")
        .expect("`name` will be there if route was matched")
        .to_string();

    let template = HelloTemplate { name };

    HtmlTemplate(template)
}

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate {
    name: String,
}

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> http::Response<Body> {
        match self.0.render() {
            Ok(html) => response::Html(html).into_response(),
            Err(err) => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!(
                    "Failed to render template. Error: {}",
                    err
                )))
                .unwrap(),
        }
    }
}
