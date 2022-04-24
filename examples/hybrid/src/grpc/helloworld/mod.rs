mod server;

use server::{rpc_helloworld::greeter_server::GreeterServer, GrpcServiceImpl};

pub fn make_service() -> GreeterServer<GrpcServiceImpl> {
    GreeterServer::new(GrpcServiceImpl::default())
}
