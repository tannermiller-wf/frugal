extern crate frugal;
extern crate nats;
extern crate rust;
extern crate thrift;

use thrift::protocol::{TCompactInputProtocolFactory, TCompactOutputProtocolFactory};

use frugal::context::FContext;
use frugal::protocol::{FInputProtocolFactory, FOutputProtocolFactory};
use frugal::transport::nats::FNatsServerBuilder;

use rust::basefoo_service::{FBaseFoo, FBaseFooProcessorBuilder};

#[derive(Clone)]
struct FBaseFooImpl;

impl FBaseFoo for FBaseFooImpl {
    fn base_ping(&mut self, ctx: &FContext) -> thrift::Result<()> {
        println!("base_ping(): {}", ctx.correlation_id());
        Ok(())
    }
}

fn main() {
    let iprot_factory = FInputProtocolFactory::new(TCompactInputProtocolFactory::new());
    let oprot_factory = FOutputProtocolFactory::new(TCompactOutputProtocolFactory::new());

    let processor = FBaseFooProcessorBuilder::new(FBaseFooImpl).build();

    let mut server = FNatsServerBuilder::new(
        "theserver:5000",
        processor,
        iprot_factory,
        oprot_factory,
        vec!["subjects".to_string()],
    )
    .build();

    if let Err(err) = server.serve() {
        println!("exiting with an error: {}", err);
    }
}
