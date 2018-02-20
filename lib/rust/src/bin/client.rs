extern crate byteorder;
extern crate frugal;
#[macro_use]
extern crate hyper;
extern crate pretty_env_logger;
extern crate thrift;
extern crate tokio_core;

use byteorder::{BigEndian, WriteBytesExt};
use hyper::Client;
use hyper::header::{qitem, Accept, ContentType, Headers};
use tokio_core::reactor::Core;

// TODO: This is unwieldy to import both everywhere, consider if we need the trait
use frugal::context::{FContext, FContextImpl};
use frugal::transport::FTransport;
use frugal::transport::http::{ContentTransferEncodingHeader, FHttpTransportBuilder};

header!{ (Foo, "foo") => [String] }
header!{ (FirstHeader, "first-header") => [String] }
header!{ (SecondHeader, "second-header") => [String] }

fn get_request_headers(fctx: &FContextImpl) -> Headers {
    let mut headers = Headers::new();
    headers.set(FirstHeader(fctx.correlation_id().to_string()));
    headers.set(SecondHeader("yup".to_string()));
    headers.set(ContentType("these/headers".parse().unwrap()));
    headers.set(Accept(vec![qitem("should/be".parse().unwrap())]));
    headers.set(ContentTransferEncodingHeader("overwritten".to_string()));
    headers
}

fn prepend_frame_size(bs: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.write_u32::<BigEndian>(bs.len() as u32).unwrap();
    frame.extend(bs);
    frame
}

fn main() {
    pretty_env_logger::init();

    let request_bytes = "Hello from the other side".as_bytes();
    let framed_request_bytes = prepend_frame_size(request_bytes);

    // TODO: why can't I hang the "with" method right off of the new() call?
    let core = Core::new().unwrap();
    let mut builder = FHttpTransportBuilder::new(
        Client::new(&core.handle()),
        core,
        "http://127.0.0.1:1234".parse().unwrap(),
    );
    builder
        .with_request_headers({
            let mut request_headers = Headers::new();
            request_headers.set(Foo("bar".to_string()));
            request_headers
        })
        .with_request_headers_from_fcontext(Box::new(get_request_headers));
    let mut transport = builder.build();

    let fctx = FContextImpl::new(None);

    println!("starting real request");
    let mut foo = "".to_string();
    transport
        .request(&fctx, &framed_request_bytes)
        .unwrap()
        .unwrap()
        .read_to_string(&mut foo);
    println!("after request {}", foo);
}
