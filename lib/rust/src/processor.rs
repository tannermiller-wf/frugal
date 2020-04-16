use async_trait::async_trait;
use thrift;
use thrift::transport::{TReadTransport, TWriteTransport};

use crate::protocol::{FInputProtocol, FOutputProtocol};

#[async_trait]
pub trait FProcessor: Clone + Send + Sync + 'static {
    async fn process<R, W>(
        &self,
        iprot: &mut FInputProtocol<R>,
        oprot: &mut FOutputProtocol<W>,
    ) -> thrift::Result<()>
    where
        R: TReadTransport + Send,
        W: TWriteTransport + Send;

    //fn annotations(&self) -> &BTreeMap<String, BTreeMap<String, String>>;
}
