use axum::prelude::*;
use bytes::BytesMut;
use futures::stream::StreamExt;
use http::StatusCode;
use std::{borrow::Cow, net::SocketAddr};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with some routes
    let app = route("/", get(show_form).post(accept_form))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    hyper::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn show_form() -> response::Html<&'static str> {
    response::Html(
        r#"
        <!doctype html>
        <html>
            <head></head>
            <body>
                <form action="/" method="post" enctype="multipart/form-data">
                    <label>
                        Upload file:
                        <input type="file" name="file" multiple>
                    </label>

                    <input type="submit" value="Upload files">
                </form>
            </body>
        </html>
        "#,
    )
}

async fn accept_form(
    mut multipart: extract::Multipart,
) -> Result<(), (StatusCode, Cow<'static, str>)> {
    while let Some(part) = multipart.next_part().await {
        let mut part = part.map_err(|err| bad_request(err.to_string()))?;

        println!(
            "input field name = {}",
            std::str::from_utf8(part.name()).map_err(|err| bad_request(err.to_string()))?
        );

        if let Some(filename) = part.filename().map(std::str::from_utf8) {
            let filename = filename.map_err(|_| bad_request("filename is not invalid utf-8"))?;
            println!("filename = {}", filename);
        }

        let mut data = BytesMut::new();
        while let Some(chunk) = part.next().await {
            let chunk = chunk.map_err(|err| bad_request(err.to_string()))?;
            data.extend_from_slice(&chunk);
        }
        println!("file length = {} bytes", data.len());
    }

    Ok(())
}

fn bad_request<S>(reason: S) -> (StatusCode, Cow<'static, str>)
where
    S: Into<Cow<'static, str>>,
{
    (StatusCode::BAD_REQUEST, reason.into())
}
