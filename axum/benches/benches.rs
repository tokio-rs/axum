use axum::{
    routing::{get, post},
    Json, Router, Server,
};
use hyper::server::conn::AddrIncoming;
use serde::{Deserialize, Serialize};
use std::{
    io::BufRead,
    process::{Command, Stdio},
};

fn main() {
    ensure_rewrk_is_installed();

    benchmark("minimal").run(Router::new);

    benchmark("basic").run(|| Router::new().route("/", get(|| async { "Hello, World!" })));

    benchmark("routing").path("/foo/bar/baz").run(|| {
        let mut app = Router::new();
        for a in 0..10 {
            for b in 0..10 {
                for c in 0..10 {
                    app = app.route(&format!("/foo-{}/bar-{}/baz-{}", a, b, c), get(|| async {}));
                }
            }
        }
        app.route("/foo/bar/baz", get(|| async {}))
    });

    benchmark("receive-json")
        .method("post")
        .headers(&[("content-type", "application/json")])
        .body(r#"{"n": 123, "s": "hi there", "b": false}"#)
        .run(|| Router::new().route("/", post(|_: Json<Payload>| async {})));

    benchmark("send-json").run(|| {
        Router::new().route(
            "/",
            get(|| async {
                Json(Payload {
                    n: 123,
                    s: "hi there".to_owned(),
                    b: false,
                })
            }),
        )
    });
}

#[derive(Deserialize, Serialize)]
struct Payload {
    n: u32,
    s: String,
    b: bool,
}

fn benchmark(name: &'static str) -> BenchmarkBuilder {
    BenchmarkBuilder {
        name,
        path: None,
        method: None,
        headers: None,
        body: None,
    }
}

struct BenchmarkBuilder {
    name: &'static str,
    path: Option<&'static str>,
    method: Option<&'static str>,
    headers: Option<&'static [(&'static str, &'static str)]>,
    body: Option<&'static str>,
}

macro_rules! config_method {
    ($name:ident, $ty:ty) => {
        fn $name(mut self, $name: $ty) -> Self {
            self.$name = Some($name);
            self
        }
    };
}

impl BenchmarkBuilder {
    config_method!(path, &'static str);
    config_method!(method, &'static str);
    config_method!(headers, &'static [(&'static str, &'static str)]);
    config_method!(body, &'static str);

    fn run<F>(self, f: F)
    where
        F: FnOnce() -> Router,
    {
        // support only running some benchmarks with
        // ```
        // cargo bench -- routing send-json
        // ```
        let args = std::env::args().collect::<Vec<_>>();
        let names = &args[1..args.len() - 1];
        if !names.is_empty() && !names.contains(&self.name.to_owned()) {
            return;
        }

        let app = f();

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let listener = rt
            .block_on(tokio::net::TcpListener::bind("0.0.0.0:0"))
            .unwrap();
        let addr = listener.local_addr().unwrap();

        std::thread::spawn(move || {
            rt.block_on(async move {
                let incoming = AddrIncoming::from_listener(listener).unwrap();
                Server::builder(incoming)
                    .serve(app.into_make_service())
                    .await
                    .unwrap();
            });
        });

        let mut cmd = Command::new("rewrk");
        cmd.stdout(Stdio::piped());

        cmd.arg("--host");
        if let Some(path) = self.path {
            cmd.arg(format!("http://{}{}", addr, path));
        } else {
            cmd.arg(format!("http://{}", addr));
        }

        cmd.args(&["--connections", "10"]);
        cmd.args(&["--threads", "10"]);
        cmd.args(&["--duration", "10s"]);

        if let Some(method) = self.method {
            cmd.arg("--method");
            cmd.arg(method);
        }

        for (key, value) in self.headers.into_iter().flatten() {
            cmd.arg("--header");
            cmd.arg(format!("{}: {}", key, value));
        }

        if let Some(body) = self.body {
            cmd.arg("--body");
            cmd.arg(body);
        }

        eprintln!("Running {:?} benchmark", self.name);

        // indent output from `rewrk` so its easier to read when running multiple benchmarks
        let mut child = cmd.spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stdout = std::io::BufReader::new(stdout);
        for line in stdout.lines() {
            let line = line.unwrap();
            println!("  {}", line);
        }

        let status = child.wait().unwrap();

        if !status.success() {
            eprintln!("`rewrk` command failed");
            std::process::exit(status.code().unwrap());
        }
    }
}

fn ensure_rewrk_is_installed() {
    let mut cmd = Command::new("rewrk");
    cmd.arg("--help");
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    let status = cmd.status().unwrap();
    if !status.success() {
        eprintln!("rewrk is not installed. See https://github.com/lnx-search/rewrk");
        std::process::exit(1);
    }
}
