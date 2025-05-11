# axum

🌍 اللغات المتاحة:  
🇺🇸 [English](README.md) | 🇷🇺 [Russian](README.ru.md) | 🇨🇳 [Chinese](README.zh.md) | 🇪🇬 [Arabic](README.ar.md) | 🇮🇳 [Hindi](README.hi.md) | 🇯🇵 [Japanese](README.ja.md)

`axum` هو إطار عمل لتطبيقات الويب، بسيط وقابل للتعديل.

[![Build Status](https://github.com/tokio-rs/axum/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/tokio-rs/axum/actions/workflows/CI.yml)
[![Crates.io](https://img.shields.io/crates/v/axum)](https://crates.io/crates/axum)
[![Documentation](https://docs.rs/axum/badge.svg)](https://docs.rs/axum)

لمزيد من المعلومات حول هذه الحزمة، يرجى الاطلاع على [توثيق الحزمة][docs].

## الميزات الرئيسية

- توجيه الطلبات إلى دوال المعالجة باستخدام واجهة برمجة التطبيقات (API) البسيطة دون الحاجة إلى استخدام الماكروهات.
- استخدام أدوات استخراج لتفسير البيانات من الطلبات بطريقة مرنة وسهلة.
- نموذج معالجة الأخطاء البسيط والمتوقع.
- القدرة على توليد الاستجابات مع الحد الأدنى من كود الـ Boilerplate.
- استخدام كامل لـ [`tower`] و [`tower-http`] ضمن النظام البيئي للخدمات، وأدوات التعامل مع البيانات، والـ middleware.

النقطة الأخيرة تميز `axum` عن غيره من الإطارات. لا يحتوي `axum` على نظام الـ middleware الخاص به، بل يعتمد على [`tower::Service`] لإدارة الـ middleware. هذا يعني أن `axum` يأتي مع أدوات جاهزة للـ timeouts، التتبع، الضغط، التوثيق، وغير ذلك، بالإضافة إلى إمكانية مشاركة هذه الأدوات مع التطبيقات التي تستخدم [`hyper`] أو [`tonic`].

## ⚠️ التغييرات المدمرة ⚠️

نحن نتحرك نحو إصدار `axum` 0.9، وقد تحتوي فرع `main` على تغييرات مدمرة. إذا كنت تستخدم الإصدار المتاح على [crates.io]، يمكنك الرجوع إلى فرع [`0.8.x`] بدلاً من ذلك.

[`0.8.x`]: https://github.com/tokio-rs/axum/tree/v0.8.x

## مثال للاستخدام

````rust
use axum::{
    routing::{get, post},
    http::StatusCode,
    Json, Router,
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    // تهيئة التتبع
    tracing_subscriber::fmt::init();

    // إعداد التطبيق وإضافة التوجيهات
    let app = Router::new()
        // تحديد معالج الطلبات لـ `GET /`
        .route("/", get(root))
        // تحديد معالج الطلبات لـ `POST /users`
        .route("/users", post(create_user));

    // بدء التطبيق على المنفذ 3000 باستخدام `hyper`
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// معالج بسيط يُرجع نصًا ثابتًا
async fn root() -> &'static str {
    "Hello, World!"
}

async fn create_user(
    // هذا الوسيط يتيح لـ `axum` استخراج البيانات من جسم الطلب وتحويلها إلى نوع `CreateUser`
    Json(payload): Json<CreateUser>,
) -> (StatusCode, Json<User>) {
    // منطق التطبيق هنا
    let user = User {
        id: 1337,
        username: payload.username,
    };

    // إرجاع استجابة بصيغة JSON مع حالة `201 Created`
    (StatusCode::CREATED, Json(user))
}

// بيانات الإدخال لـ `create_user`
#[derive(Deserialize)]
struct CreateUser {
    username: String,
}

// بيانات الخرج لـ `create_user`
#[derive(Serialize)]
struct User {
    id: u64,
    username: String,
}
```

```rust

لمزيد من الأمثلة، يمكنك التحقق من [التوثيق][docs] أو الاطلاع على أمثلة كاملة في \[دليل الاستخدام].

## الأداء

`axum` هو إطار عمل خفيف جدًا يعتمد على `hyper`، ويعني أن الأداء الذي يقدمه `axum` مماثل لذلك الذي تقدمه `hyper`. يمكنك التحقق من [القياسات هنا](https://github.com/programatik29/rust-web-benchmarks) و [هنا](https://web-frameworks-benchmark.netlify.app/result?l=rust).

## الأمان

تستخدم هذه الحزمة `#![forbid(unsafe_code)]`، مما يعني أن جميع تنفيذاتها هي أكواد آمنة 100% في لغة رست.

## الحد الأدنى للإصدار المدعوم من Rust

أقل إصدار مدعوم من `rust` هو 1.75.

## أمثلة

تتضمن مجلدات \[examples] في هذا المشروع العديد من الأمثلة لاستخدام `axum`. للحصول على تعليمات أكثر تفصيلاً، يمكنك الاطلاع على [دليل الاستخدام][docs].

## المساعدة

إذا كنت بحاجة إلى مساعدة، يمكنك طرح أسئلتك في [دردشة Discord][chat] أو فتح موضوع في [المناقشات][discussion].

## المشاريع المجتمعية

للاطلاع على مشاريع المجتمع التي تستخدم `axum`، قم بزيارة [هنا][ecosystem].

## المساهمة

🎉 نرحب بمساهماتك! إذا كنت ترغب في المساهمة في مشروع `axum`، يرجى الاطلاع على [دليل المساهمة][contributing].

## الترخيص

يتم إصدار هذا المشروع تحت [رخصة MIT][license].

### المساهمة

ما لم تحدد بخلاف ذلك، فإن جميع المساهمات في `axum` يتم ترخيصها بموجب رخصة MIT، دون أي شروط إضافية.

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

````
