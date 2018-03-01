extern crate byteorder;
extern crate frugal;
extern crate hyper;
extern crate pretty_env_logger;
extern crate tokio_core;

use byteorder::{BigEndian, WriteBytesExt};
use hyper::Client;
use tokio_core::reactor::Core;

use frugal::context::FContext;
use frugal::transport::FTransport;
use frugal::transport::http::FHttpTransportBuilder;

fn prepend_frame_size(bs: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.write_u32::<BigEndian>(bs.len() as u32).unwrap();
    frame.extend(bs);
    frame
}

fn main() {
    let request_bytes = "Hello from the other side".as_bytes();
    let framed_request_bytes = prepend_frame_size(request_bytes);

    let core = Core::new().unwrap();
    let mut transport = FHttpTransportBuilder::new(
        Client::new(&core.handle()),
        core,
        "http://localhost:9090".parse().unwrap(),
    ).build();

    let fctx = FContext::new(None);

    println!("starting real request");
    let mut foo = "".to_string();
    transport
        .request(&fctx, &framed_request_bytes)
        .unwrap()
        .unwrap()
        .read_to_string(&mut foo)
        .unwrap();
    println!("{}", &foo);
}
