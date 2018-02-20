extern crate frugal;
extern crate hyper;
extern crate pretty_env_logger;
extern crate thrift;

use std::sync::Arc;

use hyper::server::Http;
use thrift::protocol::{TCompactInputProtocolFactory, TCompactOutputProtocolFactory,
                       TOutputProtocol};

//use frugal::context::{FContext, FContextImpl};
use frugal::processor::FProcessor;
use frugal::protocol::{FInputProtocol, FInputProtocolFactory, FOutputProtocol};
use frugal::transport::http::FHttpService;

struct MockProcessor;
impl FProcessor for MockProcessor {
    fn process(&self, _: &mut FInputProtocol, oprot: &mut FOutputProtocol) -> thrift::Result<()> {
        oprot.write_string("Hello, World!")?;
        oprot.flush()
    }
}

fn main() {
    pretty_env_logger::init();

    let addr = "127.0.0.1:1234".parse().unwrap();
    let iprot = Arc::new(FInputProtocolFactory::new(Box::new(
        TCompactInputProtocolFactory::new(),
    )));
    let oprot = Arc::new(frugal::protocol::FOutputProtocolFactory::new(Box::new(
        TCompactOutputProtocolFactory::new(),
    )));
    let service = FHttpService::new(Arc::new(MockProcessor), iprot, oprot);
    let server = Http::new().bind(&addr, service).unwrap();
    server.run().unwrap();
}
