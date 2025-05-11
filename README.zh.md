# axum

🌍 可用语言：  
🇺🇸 [英语](README.md) | 🇷🇺 [俄语](README.ru.md) | 🇨🇳 [中文](README.zh.md) | 🇸🇦 [阿拉伯语](README.ar.md) | 🇮🇳 [印地语](README.hi.md) | 🇯🇵 [日语](README.ja.md)

`axum` 是一个专注于易用性和模块化的 Web 应用框架。

[![构建状态](https://github.com/tokio-rs/axum/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/tokio-rs/axum/actions/workflows/CI.yml)
[![Crates.io](https://img.shields.io/crates/v/axum)](https://crates.io/crates/axum)
[![文档](https://docs.rs/axum/badge.svg)](https://docs.rs/axum)

更多关于这个 crate 的信息可以在 [crate 文档][docs] 中找到。

## 主要特性

- 使用宏自由的 API 将请求路由到处理函数。
- 使用提取器声明性地解析请求。
- 简单且可预测的错误处理模型。
- 通过最小的样板代码生成响应。
- 完全利用 [`tower`] 和 [`tower-http`] 生态系统中的中间件、服务和工具。

特别是最后一点使得 `axum` 与其他框架区别开来。
`axum` 没有自己的中间件系统，而是使用 [`tower::Service`]。这意味着 `axum` 可以免费获得超时、追踪、压缩、授权等功能。它还允许你与使用 [`hyper`] 或 [`tonic`] 编写的应用程序共享中间件。

## ⚠ 破坏性更改 ⚠

我们正在朝着 axum 0.9 版本努力，因此 `main` 分支包含破坏性更改。查看 [`0.8.x`] 分支了解已发布到 crates.io 的版本。

[`0.8.x`]: https://github.com/tokio-rs/axum/tree/v0.8.x

## 使用示例

````rust
use axum::{
    routing::{get, post},
    http::StatusCode,
    Json, Router,
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    // 初始化 tracing
    tracing_subscriber::fmt::init();

    // 构建我们的应用程序并设置路由
    let app = Router::new()
        // `GET /` 路由到 `root` 处理函数
        .route("/", get(root))
        // `POST /users` 路由到 `create_user` 处理函数
        .route("/users", post(create_user));

    // 使用 hyper 启动应用并监听 3000 端口
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// 基本处理函数，返回静态字符串
async fn root() -> &'static str {
    "Hello, World!"
}

async fn create_user(
    // 这个参数告诉 axum 解析请求体
    // 为 JSON 格式并转化为 `CreateUser` 类型
    Json(payload): Json<CreateUser>,
) -> (StatusCode, Json<User>) {
    // 在这里插入应用逻辑
    let user = User {
        id: 1337,
        username: payload.username,
    };

    // 返回 JSON 响应并设置状态码为 `201 Created`
    (StatusCode::CREATED, Json(user))
}

// `create_user` 处理函数的输入数据
#[derive(Deserialize)]
struct CreateUser {
    username: String,
}

// `create_user` 处理函数的输出数据
#[derive(Serialize)]
struct User {
    id: u64,
    username: String,
}
```rust

````

你可以在 [example][readme-example] 和其他示例项目中找到这个 [示例][examples]。

有关更多示例，请查看 [crate 文档][docs]。

## 性能

`axum` 是一个相对轻量的框架，基于 [`hyper`]，因此它几乎不会增加额外的开销。因此，`axum` 的性能与 [`hyper`] 相当。你可以在 [这里](https://github.com/programatik29/rust-web-benchmarks) 和 [这里](https://web-frameworks-benchmark.netlify.app/result?l=rust) 查找基准测试。

## 安全性

此 crate 使用 `#![forbid(unsafe_code)]` 来确保所有实现都在 100% 安全的 Rust 代码中完成。

## 最低支持 Rust 版本

axum 的最低支持 Rust 版本是 1.75。

## 示例

[examples] 文件夹包含了多种使用 `axum` 的示例。文档中也提供了很多代码片段和示例。想要了解更多完整示例，查看社区维护的 \[展示项目] 或 \[教程]。

## 获取帮助

在 `axum` 的仓库中，我们也有许多示例 [examples] 展示了如何将所有内容组合起来。社区维护的 \[展示项目] 和 \[教程] 也展示了如何在实际应用中使用 `axum`。如果你有问题，欢迎在 [Discord 频道][chat] 提问，或者打开一个 \[讨论]。

## 社区项目

查看 [这里][ecosystem] 获取社区维护的 crate 和使用 `axum` 开发的项目列表。

## 贡献

🎈 感谢你为改进项目所做的贡献！我们非常高兴能有你加入！我们有一个 [贡献指南][contributing] 来帮助你参与 `axum` 项目。

## 许可证

本项目使用 [MIT 许可证][license]。

### 贡献

除非你明确声明，否则任何为 `axum` 提交的贡献，都会被许可为 MIT 许可证，且没有其他附加条款或条件。

[readme-example]: https://github.com/tokio-rs/axum/tree/main/examples/readme
[examples]: https://github.com/tokio-rs/axum/tree/main/examples
[docs]: https://docs.rs/axum
[`tower`]: https://crates.io/crates/tower
[`hyper`]: https://crates.io/crates/hyper
[`tower-http`]: https://crates.io/crates/tower-http
[`tonic`]: https://crates.io/crates/tonic
[contributing]: https://github.com/tokio-rs/axum/blob/main/CONTRIBUTING.md
[chat]: https://discord.gg/tokio
[discussion]: https://github.com/tokio-rs/axum/discussions/new?category=q-a
[`tower::Service`]: https://docs.rs/tower/latest/tower/trait.Service.html
[ecosystem]: https://github.com/tokio-rs/axum/blob/main/ECOSYSTEM.md
[showcases]: https://github.com/tokio-rs/axum/blob/main/ECOSYSTEM.md#project-showcase
[tutorials]: https://github.com/tokio-rs/axum/blob/main/ECOSYSTEM.md#tutorials
[license]: https://github.com/tokio-rs/axum/blob/main/axum/LICENSE
