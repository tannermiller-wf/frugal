use thrift;

use context::FContext;
use protocol::{FInputProtocol, FOutputProtocol};

pub trait FProcessor {
    fn process(
        &mut self,
        iprot: &mut FInputProtocol,
        oprot: &mut FOutputProtocol,
    ) -> thrift::Result<()>;

    //fn annotations(&self) -> &BTreeMap<String, BTreeMap<String, String>>;
}

pub trait FProcessorFunction {
    fn process(
        &self,
        ctx: &mut FContext,
        iprot: &mut FInputProtocol,
        oprot: &mut FOutputProtocol,
    ) -> thrift::Result<()>;

    //fn annotations(&self) -> &BTreeMap<String, BTreeMap<String, String>>;
}

pub trait FBaseProcessor {
    fn processor(&self, name: &str) -> Option<&dyn FProcessorFunction>;

    //fn annotations(&self) -> &BTreeMap<String, BTreeMap<String, String>>;
}
