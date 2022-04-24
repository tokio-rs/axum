use rpc_helloworld::{greeter_server::Greeter, HelloReply, HelloRequest};
use tonic::{Response, Status};

pub mod rpc_helloworld {
    tonic::include_proto!("helloworld");
}

#[derive(Default)]
pub struct GrpcServiceImpl {}

#[tonic::async_trait]
impl Greeter for GrpcServiceImpl {
    async fn say_hello(
        &self,
        request: tonic::Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        println!("Got a request from {:?}", request.remote_addr());

        let reply = HelloReply {
            message: format!("Hello {}!", request.into_inner().name),
        };
        Ok(Response::new(reply))
    }
}
