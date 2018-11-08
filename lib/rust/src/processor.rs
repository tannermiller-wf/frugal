use thrift;

use protocol::{FInputProtocol, FOutputProtocol};

pub trait FProcessor {
    fn process(
        &mut self,
        iprot: &mut FInputProtocol,
        oprot: &mut FOutputProtocol,
    ) -> thrift::Result<()>;

    //fn annotations(&self) -> &BTreeMap<String, BTreeMap<String, String>>;
}
