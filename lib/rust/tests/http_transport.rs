extern crate byteorder;
extern crate frugal;
extern crate futures;
#[macro_use]
extern crate hyper;
extern crate pretty_env_logger;
extern crate thrift;
extern crate tokio_core;

use std::sync::Arc;
use std::sync::mpsc::sync_channel;
use std::thread;

use byteorder::{BigEndian, WriteBytesExt};
use futures::future::{lazy, ok, Future};
use futures::sync::oneshot::channel;
use hyper::Client;
use hyper::header::{qitem, Accept, ContentType, Headers};
use hyper::server::Http;
use thrift::protocol::{TCompactInputProtocolFactory, TCompactOutputProtocolFactory,
                       TOutputProtocol};
use tokio_core::reactor::Core;

// TODO: This is unwieldy to import both everywhere, consider if we need the trait
use frugal::context::{FContext, FContextImpl};
use frugal::processor::FProcessor;
use frugal::protocol::{FInputProtocol, FInputProtocolFactory, FOutputProtocol};
use frugal::transport::FTransport;
use frugal::transport::http::{ContentTransferEncodingHeader, FHttpService, FHttpTransportBuilder};

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

struct MockProcessor;
impl FProcessor for MockProcessor {
    fn process(&self, _: &mut FInputProtocol, oprot: &mut FOutputProtocol) -> thrift::Result<()> {
        oprot.write_string("Hello, World!")?;
        oprot.flush()
    }
}

fn prepend_frame_size(bs: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.write_u32::<BigEndian>(bs.len() as u32).unwrap();
    frame.extend(bs);
    frame
}

static ADDRESS: &'static str = "127.0.0.1:1234";

#[test]
fn test_http_request_headers_with_context() {
    pretty_env_logger::init();

    let request_bytes = "Hello from the other side".as_bytes();
    let framed_request_bytes = prepend_frame_size(request_bytes);

    let (setup_sender, setup_receiver) = sync_channel(0);
    let (kill_sender, kill_receiver) = channel();

    // set up server in another thread
    thread::spawn(move || {
        let addr = ADDRESS.parse().unwrap();
        let iprot = Arc::new(FInputProtocolFactory::new(Box::new(
            TCompactInputProtocolFactory::new(),
        )));
        let oprot = Arc::new(frugal::protocol::FOutputProtocolFactory::new(Box::new(
            TCompactOutputProtocolFactory::new(),
        )));
        let service = FHttpService::new(Arc::new(MockProcessor), iprot, oprot);
        let server = Http::new().bind(&addr, service).unwrap();
        server
            .run_until(
                lazy(|| {
                    setup_sender.send(()).unwrap();
                    ok::<(), ()>(())
                }).then(|_| kill_receiver.map_err(|_| ())),
            )
            .unwrap();
    });

    let core = Core::new().unwrap();

    // TODO: why can't I hang the "with" method right off of the new() call?
    let mut builder = FHttpTransportBuilder::new(
        Client::new(&core.handle()),
        core,
        format!("http://{}", ADDRESS).parse().unwrap(),
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

    setup_receiver.recv().unwrap();

    match transport.request(&fctx, &[0u8, 0, 0, 0]) {
        None => (),
        Some(_) => panic!("expected None, got Some"),
    };

    let mut str_from_proc = "".to_string();
    let out = transport
        .request(&fctx, &framed_request_bytes)
        .unwrap()
        .unwrap()
        .read_to_string(&mut str_from_proc)
        .unwrap();

    assert_eq!(18, out);

    // use index 5 to skip the protocol version byte and size u32
    assert_eq!("Hello, World!", &str_from_proc[5..]);

    kill_sender.send(()).unwrap();
}