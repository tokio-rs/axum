fn main() {
    tonic_build::configure()
        .type_attribute(".", r#"#[derive(serde::Serialize, serde::Deserialize)]"#)
        .compile(&["proto/helloworld.proto"], &["proto"]).unwrap();
}
