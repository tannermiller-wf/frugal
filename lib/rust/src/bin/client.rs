extern crate byteorder;
extern crate frugal;
extern crate hyper;
extern crate tokio_core;

use byteorder::{BigEndian, WriteBytesExt};
use hyper::Client;
use tokio_core::reactor::Core;

use frugal::context::FContext;
use frugal::transport::http::FHttpTransportBuilder;
use frugal::transport::FTransport;

// TODO: Consider moving this to its own crate with own Cargo.toml

fn prepend_frame_size(bs: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.write_u32::<BigEndian>(bs.len() as u32).unwrap();
    frame.extend(bs);
    frame
}

fn main() {
    let request_bytes = b"Hello from the other side";
    let framed_request_bytes = prepend_frame_size(request_bytes);

    let core = Core::new().unwrap();
    let mut transport = FHttpTransportBuilder::new(
        Client::new(&core.handle()),
        core,
        "http://localhost:9090".parse().unwrap(),
    ).build();

    let fctx = FContext::new(None);

    println!("starting real request");
    let mut payload = "".to_string();
    transport
        .request(&fctx, &framed_request_bytes)
        .unwrap()
        .unwrap()
        .read_to_string(&mut payload)
        .unwrap();
    println!("{}", &payload);
}
