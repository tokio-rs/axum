fn main() {
    tonic_build::configure()
        // make the gRPC message structs serializable to support the json_wrap_grpc function
        .type_attribute(".", r#"#[derive(serde::Serialize, serde::Deserialize)]"#)
        .compile(&["proto/helloworld.proto"], &["proto"]).unwrap();
}
