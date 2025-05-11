# axum

🌍 使用可能な言語：  
🇺🇸 [英語](README.md) | 🇷🇺 [ロシア語](README.ru.md) | 🇨🇳 [中国語](README.zh.md) | 🇸🇦 [アラビア語](README.ar.md) | 🇮🇳 [ヒンディー語](README.hi.md) | 🇯🇵 [日本語](README.ja.md)

`axum` は、使いやすさとモジュール化を重視した Web アプリケーションフレームワークです。

[![Build Status](https://github.com/tokio-rs/axum/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/tokio-rs/axum/actions/workflows/CI.yml)
[![Crates.io](https://img.shields.io/crates/v/axum)](https://crates.io/crates/axum)
[![Documentation](https://docs.rs/axum/badge.svg)](https://docs.rs/axum)

このクレートの詳細については、[crate ドキュメント][docs]をご覧ください。

## 主な特徴

- マクロを使わずに、リクエストを処理関数にルーティングするシンプルな API。
- 宣言的にリクエストを解析するためのエクストラクタ。
- シンプルで予測可能なエラーハンドリングモデル。
- 最小限のボイラープレートコードでレスポンスを生成。
- 完全に [`tower`] と [`tower-http`] エコシステム内のミドルウェア、サービス、ツールを活用。

特に最後のポイントが、`axum` を他のフレームワークと差別化しています。
`axum` には独自のミドルウェアシステムがない代わりに、[`tower::Service`] を使用しています。これにより、`axum` はタイムアウト、トラッキング、圧縮、認証などの機能を無料で利用できます。また、[`hyper`] や [`tonic`] で書かれたアプリケーションとのミドルウェア共有も可能です。

## ⚠ 破壊的変更 ⚠

私たちは `axum` のバージョン 0.9 を目指しており、`main` ブランチには破壊的変更が含まれています。crates.io に公開されたバージョンについては、[`0.8.x`] ブランチを参照してください。

[`0.8.x`]: https://github.com/tokio-rs/axum/tree/v0.8.x

## 使用例

````rust
use axum::{
    routing::{get, post},
    http::StatusCode,
    Json, Router,
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    // tracing の初期化
    tracing_subscriber::fmt::init();

    // アプリケーションを構築し、ルーティングを設定
    let app = Router::new()
        // `GET /` に `root` ハンドラをマッピング
        .route("/", get(root))
        // `POST /users` に `create_user` ハンドラをマッピング
        .route("/users", post(create_user));

    // hyper を使ってアプリケーションを 3000 番ポートで開始
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// 基本的なハンドラ関数、静的な文字列を返す
async fn root() -> &'static str {
    "Hello, World!"
}

async fn create_user(
    // この引数により axum はリクエストボディを解析し、`CreateUser` 型に変換します
    Json(payload): Json<CreateUser>,
) -> (StatusCode, Json<User>) {
    // アプリケーションのロジックをここに記述
    let user = User {
        id: 1337,
        username: payload.username,
    };

    // JSON レスポンスを返し、ステータスコード `201 Created` を設定
    (StatusCode::CREATED, Json(user))
}

// `create_user` ハンドラの入力データ
#[derive(Deserialize)]
struct CreateUser {
    username: String,
}

// `create_user` ハンドラの出力データ
#[derive(Serialize)]
struct User {
    id: u64,
    username: String,
}
```rust

````

この [example][readme-example] やその他のサンプルプロジェクトで、このコードの使い方を確認できます。

その他のサンプルについては、[crate ドキュメント][docs]をご覧ください。

## パフォーマンス

`axum` は非常に軽量なフレームワークで、`hyper` に基づいているため、ほとんどオーバーヘッドはありません。つまり、`axum` のパフォーマンスは `hyper` と同等です。[こちら](https://github.com/programatik29/rust-web-benchmarks) と [こちら](https://web-frameworks-benchmark.netlify.app/result?l=rust) でベンチマークを確認できます。

## セキュリティ

このクレートは、`#![forbid(unsafe_code)]` を使用しているため、すべての実装が 100% セーフ Rust コードであることが保証されています。

## 最小対応 Rust バージョン

axum の最小対応 Rust バージョンは 1.75 です。

## サンプル

[examples] フォルダには、`axum` のさまざまな使用例が含まれています。ドキュメントにも多くのコードスニペットと例があります。完全なサンプルを見たい場合は、コミュニティによってメンテナンスされている \[Showcase プロジェクト] や \[チュートリアル] を確認してください。

## ヘルプが必要な場合

`axum` のリポジトリには、すべてのコンポーネントを組み合わせた多くのサンプルがあります。[Discord チャット][chat] で質問したり、[Discussion][discussion] に投稿していただけます。

## コミュニティプロジェクト

[ここ][ecosystem] では、コミュニティがメンテナンスしている crate や、`axum` を使用したプロジェクトの一覧を確認できます。

## コントリビューション

🎉 ご協力いただきありがとうございます！私たちはあなたの参加を歓迎します！`axum` プロジェクトに参加するための [コントリビューションガイド][contributing] があります。

## ライセンス

このプロジェクトは [MIT ライセンス][license] のもとで公開されています。

### 貢献

あなたが明示的に示さない限り、`axum` に対するすべての貢献は MIT ライセンスで許可され、追加の条件はありません。

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
