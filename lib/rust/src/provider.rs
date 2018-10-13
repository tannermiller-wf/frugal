use protocol::{FInputProtocolFactory, FOutputProtocolFactory};
use tower_service::NewService;
use transport::FTransport;

pub struct FServiceProvider {
    transport: Box<dyn FTransport>,
    input_protocol_factory: FInputProtocolFactory,
    output_protocol_factory: FOutputProtocolFactory,
    //middleware: Vec<NewService>,
}

impl FServiceProvider {
    pub fn new(
        transport: Box<dyn FTransport>,
        input_protocol_factory: FInputProtocolFactory,
        output_protocol_factory: FOutputProtocolFactory,
    ) -> FServiceProvider {
        FServiceProvider {
            transport,
            input_protocol_factory,
            output_protocol_factory,
        }
    }
}
