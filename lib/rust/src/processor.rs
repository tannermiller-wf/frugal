use thrift;
use thrift::transport::{TReadTransport, TWriteTransport};

use crate::protocol::{FInputProtocol, FOutputProtocol};

pub trait FProcessor: Clone + Send + Sync + 'static {
    fn process<R, W>(
        &self,
        iprot: &mut FInputProtocol<R>,
        oprot: &mut FOutputProtocol<W>,
    ) -> thrift::Result<()>
    where
        R: TReadTransport,
        W: TWriteTransport;

    //fn annotations(&self) -> &BTreeMap<String, BTreeMap<String, String>>;
}
