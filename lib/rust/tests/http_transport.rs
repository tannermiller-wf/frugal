extern crate base64;
extern crate byteorder;
extern crate frugal;
extern crate futures;
#[macro_use]
extern crate hyper;
extern crate mime;
extern crate rand;
extern crate thrift;
extern crate tokio_core;

use std::net;
use std::sync::mpsc::sync_channel;
use std::sync::Arc;
use std::thread;

use byteorder::{BigEndian, WriteBytesExt};
use futures::future::{lazy, ok, Future};
use futures::sync::oneshot::channel;
use hyper::header::{qitem, Accept, ContentLength, ContentType, Headers};
use hyper::server::Http;
use hyper::server::Service;
use hyper::{Client, Request, Response};
use rand::random;
use thrift::protocol::{
    TCompactInputProtocolFactory, TCompactOutputProtocolFactory, TOutputProtocol,
};
use tokio_core::reactor::Core;

use frugal::context::FContext;
use frugal::processor::FProcessor;
use frugal::protocol::{FInputProtocol, FInputProtocolFactory, FOutputProtocol};
use frugal::transport::http::{
    ContentTransferEncodingHeader, FHttpService, FHttpTransportBuilder, BASE64_ENCODING,
    FRUGAL_CONTENT_TYPE,
};
use frugal::transport::FTransport;

header!{ (Foo, "foo") => [String] }
header!{ (FirstHeader, "first-header") => [String] }
header!{ (SecondHeader, "second-header") => [String] }

fn get_request_headers(fctx: &FContext) -> Headers {
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

static ADDRESS: &'static str = "127.0.0.1:123";

fn new_test_address() -> net::SocketAddr {
    loop {
        let r = random::<u8>();
        if r > 99 {
            continue;
        }
        return format!("{}{}", ADDRESS, r).parse().unwrap();
    }
}

#[test]
fn test_transport_service_integration() {
    struct MockProcessor;
    impl FProcessor for MockProcessor {
        fn process(
            &self,
            _: &mut FInputProtocol,
            oprot: &mut FOutputProtocol,
        ) -> thrift::Result<()> {
            oprot.write_string("Hello, World!")?;
            oprot.flush()
        }
    }

    let request_bytes = "Hello from the other side".as_bytes();
    let framed_request_bytes = prepend_frame_size(request_bytes);

    let (setup_sender, setup_receiver) = sync_channel(0);
    let (kill_sender, kill_receiver) = channel();

    let addr = new_test_address();

    // set up server in another thread
    let server_addr = addr.clone();
    thread::spawn(move || {
        let iprot = Arc::new(FInputProtocolFactory::new(Box::new(
            TCompactInputProtocolFactory::new(),
        )));
        let oprot = Arc::new(frugal::protocol::FOutputProtocolFactory::new(Box::new(
            TCompactOutputProtocolFactory::new(),
        )));
        let service = FHttpService::new(Arc::new(MockProcessor), iprot, oprot);
        let server = Http::new().bind(&server_addr, service).unwrap();
        server
            .run_until(
                lazy(|| {
                    setup_sender.send(()).unwrap();
                    ok::<(), ()>(())
                }).then(|_| kill_receiver.map_err(|_| ())),
            ).unwrap();
    });

    let core = Core::new().unwrap();

    let mut transport = FHttpTransportBuilder::new(
        Client::new(&core.handle()),
        core,
        format!("http://{}", addr).parse().unwrap(),
    ).with_request_headers({
        let mut request_headers = Headers::new();
        request_headers.set(Foo("bar".to_string()));
        request_headers
    }).with_request_headers_from_fcontext(Box::new(get_request_headers))
    .build();

    let fctx = FContext::new(None);

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

static RESPONSE_BODY: &'static str = "I must've called a thousand times";

#[test]
fn test_http_transport_lifecycle() {
    struct TestService;
    impl Service for TestService {
        type Request = Request;
        type Response = Response;
        type Error = hyper::Error;
        type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

        fn call(&self, req: Request) -> Self::Future {
            assert_eq!(
                &ContentType(FRUGAL_CONTENT_TYPE.clone()),
                req.headers().get::<ContentType>().unwrap()
            );
            assert_eq!(
                &ContentTransferEncodingHeader(BASE64_ENCODING.to_string()),
                req.headers()
                    .get::<ContentTransferEncodingHeader>()
                    .unwrap()
            );
            assert_eq!(
                &Accept(vec![qitem(FRUGAL_CONTENT_TYPE.clone())]),
                req.headers().get::<Accept>().unwrap()
            );

            let resp = base64::encode(&prepend_frame_size(RESPONSE_BODY.as_bytes()));
            Box::new(ok(Response::new()
                .with_header(ContentLength(resp.len() as u64))
                .with_body(resp)))
        }
    }

    let request_bytes = "Hello from the other side".as_bytes();
    let framed_request_bytes = prepend_frame_size(request_bytes);

    let (setup_sender, setup_receiver) = sync_channel(0);
    let (kill_sender, kill_receiver) = channel();

    let addr = new_test_address();

    // set up server in another thread
    let server_addr = addr.clone();
    thread::spawn(move || {
        let server = Http::new().bind(&server_addr, || Ok(TestService)).unwrap();
        server
            .run_until(
                lazy(|| {
                    setup_sender.send(()).unwrap();
                    ok::<(), ()>(())
                }).then(|_| kill_receiver.map_err(|_| ())),
            ).unwrap();
    });

    let core = Core::new().unwrap();

    let mut transport = FHttpTransportBuilder::new(
        Client::new(&core.handle()),
        core,
        format!("http://{}", addr).parse().unwrap(),
    ).build();

    let fctx = FContext::new(None);

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

    assert_eq!(37, out);

    assert_eq!(RESPONSE_BODY, &str_from_proc[4..]);

    kill_sender.send(()).unwrap();
}
