//! Run with
//!
//! ```not_rust
//! cargo run -p example-stream-to-file
//! ```

use async_stream::try_stream;
use axum::{
    body::Bytes,
    extract::{Multipart, Path, Request},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    BoxError, Router,
};
use axum_extra::response::file_stream::FileStream;
use futures::{Stream, TryStreamExt};
use std::io;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, BufWriter},
};
use tokio_util::io::{ReaderStream, StreamReader};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
const UPLOADS_DIRECTORY: &str = "uploads";

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // save files to a separate directory to not override files in the current directory
    tokio::fs::create_dir(UPLOADS_DIRECTORY)
        .await
        .expect("failed to create `uploads` directory");

    let app = Router::new()
        .route("/upload", get(show_form).post(accept_form))
        .route("/", get(show_form2).post(accept_form))
        .route("/file/{file_name}", post(save_request_body))
        .route("/file_download", get(file_download_handler))
        .route("/simpler_file_download", get(simpler_file_download_handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// Handler that streams the request body to a file.
//
// POST'ing to `/file/foo.txt` will create a file called `foo.txt`.
async fn save_request_body(
    Path(file_name): Path<String>,
    request: Request,
) -> Result<(), (StatusCode, String)> {
    stream_to_file(&file_name, request.into_body().into_data_stream()).await
}

// Handler that returns HTML for a multipart form.
async fn show_form() -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <head>
                <title>Upload something!</title>
            </head>
            <body>
                <form action="/" method="post" enctype="multipart/form-data">
                    <div>
                        <label>
                            Upload file:
                            <input type="file" name="file" multiple>
                        </label>
                    </div>

                    <div>
                        <input type="submit" value="Upload files">
                    </div>
                </form>
            </body>
        </html>
        "#,
    )
}

// Handler that returns HTML for a multipart form.
async fn show_form2() -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <head>
                <title>Upload and Download!</title>
            </head>
            <body>
                <h1>Upload and Download Files</h1>

                <!-- Form for uploading files -->
                <form action="/" method="post" enctype="multipart/form-data">
                    <div>
                        <label>
                            Upload file:
                            <input type="file" name="file" multiple>
                        </label>
                    </div>

                    <div>
                        <input type="submit" value="Upload files">
                    </div>
                </form>

                <hr>
                <!-- Buttons for downloading files -->
                <form action="/file_download" method="get">
                    <div>
                        <input type="submit" value="Download file">
                    </div>
                </form>
            </body>
        </html>
        "#,
    )
}

/// A simpler file download handler that uses the `FileStream` response.
/// Returns the entire file as a stream.
async fn simpler_file_download_handler() -> Response {
    let Ok(file) = File::open("./CHANGELOG.md").await else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to open file").into_response();
    };

    let Ok(file_metadata) = file.metadata().await else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get file metadata",
        )
            .into_response();
    };

    // Constructing a Stream with ReaderStream
    let stream = ReaderStream::new(file);

    // Use FileStream to return and set some information.
    // Will set application/octet-stream in the header.
    let file_stream_resp = FileStream::new(stream)
        .file_name("test.txt")
        .content_size(file_metadata.len());

    //It is also possible to set only the stream FileStream will be automatically set on the http header.
    //let file_stream_resp = FileStream::new(stream);

    file_stream_resp.into_response()
}

/// If you want to control the returned files in more detail you can implement a Stream
/// For example, use the try_stream! macro to construct a file stream and set which parts are needed.
async fn file_download_handler() -> Response {
    let file_stream = match try_stream("./CHANGELOG.md", 5, 25, 10).await {
        Ok(file_stream) => file_stream,
        Err(e) => {
            println!("{e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed try stream!").into_response();
        }
    };

    // Use FileStream to return and set some information.
    // Will set application/octet-stream in the header.
    let file_stream_resp = FileStream::new(Box::pin(file_stream))
        .file_name("test.txt")
        .content_size(20_u64);

    file_stream_resp.into_response()
}

/// More complex manipulation of files and conversion to a stream
async fn try_stream(
    file_path: &str,
    start: u64,
    mut end: u64,
    buffer_size: usize,
) -> Result<impl Stream<Item = Result<Vec<u8>, std::io::Error>>, String> {
    let mut file = File::open(file_path)
        .await
        .map_err(|e| format!("open file:{file_path} err:{e}"))?;

    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|e| format!("file:{file_path} seek err:{e}"))?;

    if end == 0 {
        let metadata = file
            .metadata()
            .await
            .map_err(|e| format!("file:{file_path} get metadata err:{e}"))?;
        end = metadata.len();
    }

    let mut buffer = vec![0; buffer_size];

    let stream = try_stream! {
        let mut total_read = 0;

            while total_read < end {
                let bytes_to_read = std::cmp::min(buffer_size as u64, end - total_read);
                let n = file.read(&mut buffer[..bytes_to_read as usize]).await.map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, e)
                })?;
                if n == 0 {
                    break; // EOF
                }
                total_read += n as u64;
                yield buffer[..n].to_vec();

        }
    };
    Ok(stream)
}

// Handler that accepts a multipart form upload and streams each field to a file.
async fn accept_form(mut multipart: Multipart) -> Result<Redirect, (StatusCode, String)> {
    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = if let Some(file_name) = field.file_name() {
            file_name.to_owned()
        } else {
            continue;
        };

        stream_to_file(&file_name, field).await?;
    }

    Ok(Redirect::to("/"))
}

// Save a `Stream` to a file
async fn stream_to_file<S, E>(path: &str, stream: S) -> Result<(), (StatusCode, String)>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    if !path_is_valid(path) {
        return Err((StatusCode::BAD_REQUEST, "Invalid path".to_owned()));
    }

    async {
        // Convert the stream into an `AsyncRead`.
        let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
        let body_reader = StreamReader::new(body_with_io_error);
        futures::pin_mut!(body_reader);

        // Create the file. `File` implements `AsyncWrite`.
        let path = std::path::Path::new(UPLOADS_DIRECTORY).join(path);
        let mut file = BufWriter::new(File::create(path).await?);

        // Copy the body into the file.
        tokio::io::copy(&mut body_reader, &mut file).await?;

        Ok::<_, io::Error>(())
    }
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

// to prevent directory traversal attacks we ensure the path consists of exactly one normal
// component
fn path_is_valid(path: &str) -> bool {
    let path = std::path::Path::new(path);
    let mut components = path.components().peekable();

    if let Some(first) = components.peek() {
        if !matches!(first, std::path::Component::Normal(_)) {
            return false;
        }
    }

    components.count() == 1
}
