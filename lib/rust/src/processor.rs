use thrift;

use protocol::{FInputProtocol, FOutputProtocol};

pub trait FProcessor {
    fn process(
        &self,
        input: &mut FInputProtocol,
        output: &mut FOutputProtocol,
    ) -> thrift::Result<()>;
}
