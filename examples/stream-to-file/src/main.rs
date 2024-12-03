//! Run with
//!
//! ```not_rust
//! cargo run -p example-stream-to-file
//! ```

use async_stream::try_stream;
use axum::{
    body::Bytes,
    extract::{Multipart, Path, Request},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    BoxError, Router,
};
use axum_extra::response::file_stream::{AsyncReaderStream, FileStream};
use futures::{Stream, TryStreamExt};
use std::{io, path::PathBuf};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufWriter},
};
use tokio_util::io::StreamReader;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
const UPLOADS_DIRECTORY: &str = "uploads";
const DOWNLOAD_DIRECTORY: &str = "downloads";
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

    tokio::fs::create_dir(DOWNLOAD_DIRECTORY)
        .await
        .expect("failed to create `downloads` directory");

    //create a file to download
    create_test_file(std::path::Path::new(DOWNLOAD_DIRECTORY).join("test.txt"))
        .await
        .expect("failed to create test file");

    let app = Router::new()
        .route("/upload", get(show_form).post(accept_form))
        .route("/", get(show_form2).post(accept_form))
        .route("/file/{file_name}", post(save_request_body))
        .route("/file_download", get(file_download_handler))
        .route("/simpler_file_download", get(simpler_file_download_handler))
        .route("/range_file", get(file_range_handler))
        .route("/range_file_stream", get(try_file_range_handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn create_test_file(path: PathBuf) -> io::Result<()> {
    let mut file = File::create(path).await?;
    for i in 1..=30 {
        let line = format!(
            "Hello, this is the simulated file content! This is line {}\n",
            i
        );
        file.write_all(line.as_bytes()).await?;
    }
    file.flush().await?;
    Ok(())
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

                 <!-- Button for partial file download (Range: 0-100) -->
                <form action="/range_file_stream" method="get">
                    <div>
                        <input type="hidden" name="range" value="0-100">
                        <input type="submit" value="Download range (0-100)">
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
    //If you want to simply return a file as a stream
    // you can use the from_path method directly, passing in the path of the file to construct a stream with a header and length.
    FileStream::<AsyncReaderStream>::from_path(
        &std::path::Path::new(DOWNLOAD_DIRECTORY).join("test.txt"),
    )
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to open file").into_response())
    .into_response()
}

/// If you want to control the returned files in more detail you can implement a Stream
/// For example, use the try_stream! macro to construct a file stream and set which parts are needed.
async fn file_download_handler() -> Response {
    let file_path = format!("{DOWNLOAD_DIRECTORY}/test.txt");
    let file_stream = match try_stream(&file_path, 5, 25, 10).await {
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

async fn try_stream2(
    mut file: File,
    start: u64,
    mut end: u64,
    buffer_size: usize,
) -> Result<impl Stream<Item = Result<Vec<u8>, std::io::Error>>, String> {
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|e| format!("file seek err:{e}"))?;

    if end == 0 {
        let metadata = file
            .metadata()
            .await
            .map_err(|e| format!("file  get metadata err:{e}"))?;
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

/// A file download handler that accepts a range header and returns a partial file as a stream.
/// You can return directly from the path
/// But you can't download this stream directly from your browser, you need to use a tool like curl or Postman.
async fn try_file_range_handler(headers: HeaderMap) -> Response {
    let range_header = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());

    let (start, end) = if let Some(range) = range_header {
        if let Some(range) = parse_range_header(range) {
            range
        } else {
            return (StatusCode::RANGE_NOT_SATISFIABLE, "Invalid Range").into_response();
        }
    } else {
        (0, 0) // default range end = 0, if end = 0 end == file size - 1
    };

    let file_path = format!("{DOWNLOAD_DIRECTORY}/test.txt");
    FileStream::<AsyncReaderStream>::try_range_response(
        std::path::Path::new(&file_path),
        start,
        end,
        1024,
    )
    .await
    .unwrap()
}

/// If you want to control the stream yourself
async fn file_range_handler(headers: HeaderMap) -> Response {
    // Parse the range header to get the start and end values.
    let range_header = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());

    // If the range header is invalid, return a 416 Range Not Satisfiable response.
    let (start, end) = if let Some(range) = range_header {
        if let Some(range) = parse_range_header(range) {
            range
        } else {
            return (StatusCode::RANGE_NOT_SATISFIABLE, "Invalid Range").into_response();
        }
    } else {
        (0, 0) // default range end = 0, if end = 0 end == file size - 1
    };

    let file_path = format!("{DOWNLOAD_DIRECTORY}/test.txt");

    let file = File::open(file_path).await.unwrap();

    let file_size = file.metadata().await.unwrap().len();

    let file_stream = match try_stream2(file, start, end, 256).await {
        Ok(file_stream) => file_stream,
        Err(e) => {
            println!("{e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed try stream!").into_response();
        }
    };

    FileStream::new(Box::pin(file_stream)).into_range_response(start, end, file_size)
}

/// Parse the range header and return the start and end values.
fn parse_range_header(range: &str) -> Option<(u64, u64)> {
    let range = range.strip_prefix("bytes=")?;
    let mut parts = range.split('-');
    let start = parts.next()?.parse::<u64>().ok()?;
    let end = parts
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    if start > end {
        return None;
    }
    Some((start, end))
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
