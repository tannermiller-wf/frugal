extern crate frugal;
extern crate nats;
extern crate rust;
extern crate thrift;

use thrift::protocol::{TCompactInputProtocolFactory, TCompactOutputProtocolFactory};

use frugal::context::FContext;
use frugal::protocol::{FInputProtocolFactory, FOutputProtocolFactory};
use frugal::provider::FServiceProvider;
use frugal::transport::nats::FNatsTransport;

use rust::basefoo_service::{FBaseFoo, FBaseFooClient};

fn main() {
    let transport =
        FNatsTransport::new("server::1234", None, "testing_subject", "testing_inbox").unwrap();
    let iprot_factory = FInputProtocolFactory::new(TCompactInputProtocolFactory::new());
    let oprot_factory = FOutputProtocolFactory::new(TCompactOutputProtocolFactory::new());
    let provider = FServiceProvider::new(transport, iprot_factory, oprot_factory);
    let mut client = FBaseFooClient::new(provider);

    let ctx = FContext::new(None);
    client.base_ping(&ctx).unwrap()
}
