use crate::protocol::{FInputProtocolFactory, FOutputProtocolFactory};
use crate::transport::FTransport;

#[derive(Clone)]
pub struct FServiceProvider<T>
where
    T: FTransport,
{
    pub transport: T,
    pub input_protocol_factory: FInputProtocolFactory,
    pub output_protocol_factory: FOutputProtocolFactory,
}

impl<T> FServiceProvider<T>
where
    T: FTransport,
{
    pub fn new(
        transport: T,
        input_protocol_factory: FInputProtocolFactory,
        output_protocol_factory: FOutputProtocolFactory,
    ) -> FServiceProvider<T> {
        FServiceProvider {
            transport,
            input_protocol_factory,
            output_protocol_factory,
        }
    }
}
