mod helloworld;

use tonic::transport::Server;

pub struct GrpcService;

impl GrpcService {
    pub fn build() -> tonic::transport::server::Routes {
        let hellowrld_service = helloworld::make_service();

        Server::builder()
            .add_service(hellowrld_service)
            .into_service()
    }
}
