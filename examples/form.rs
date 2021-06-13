use http::Request;
use serde::Deserialize;
use std::net::SocketAddr;
use tower_web::prelude::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = route("/", get(show_form).post(accept_form))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    app.serve(&addr).await.unwrap();
}

async fn show_form(_req: Request<Body>) -> response::Html<&'static str> {
    response::Html(
        r#"
        <!doctype html>
        <html>
            <head></head>
            <body>
                <form action="/" method="post">
                    <label for="name">
                        Enter your name:
                        <input type="text" name="name">
                    </label>

                    <label>
                        Enter your email:
                        <input type="text" name="email">
                    </label>

                    <input type="submit" value="Subscribe!">
                </form>
            </body>
        </html>
        "#,
    )
}

#[derive(Deserialize, Debug)]
struct Input {
    name: String,
    email: String,
}

async fn accept_form(extract::Form(input): extract::Form<Input>) {
    dbg!(&input);
}
