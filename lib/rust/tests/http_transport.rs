use std::net;

use byteorder::{BigEndian, WriteBytesExt};
use rand::random;
use reqwest;
use reqwest::header::{HeaderMap, HeaderValue};
use thrift;
use thrift::protocol::{
    TCompactInputProtocolFactory, TCompactOutputProtocolFactory, TOutputProtocol,
};
use thrift::transport::{TReadTransport, TWriteTransport};
use tide;
use tokio::task;

use frugal::context::FContext;
use frugal::processor::FProcessor;
use frugal::protocol::{FInputProtocol, FInputProtocolFactory, FOutputProtocol};
use frugal::transport::http::{self as fhttp, client, server};
use frugal::transport::FTransport;

static FOO: &str = "foo";
static FIRST: &str = "first";
static SECOND: &str = "second";

fn get_request_headers(fctx: &FContext) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(FIRST, HeaderValue::from_str(fctx.correlation_id()).unwrap());
    headers.insert(SECOND, HeaderValue::from_str("yup").unwrap());
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        HeaderValue::from_str("these/headers").unwrap(),
    );
    headers.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_str("should/be").unwrap(),
    );
    headers.insert(
        fhttp::CONTENT_TRANSFER_ENCODING,
        HeaderValue::from_str("overwritten").unwrap(),
    );
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

#[tokio::test(threaded_scheduler)]
async fn test_http_transport_service_integration() {
    #[derive(Clone)]
    struct MockProcessor;
    impl FProcessor for MockProcessor {
        fn process<R: TReadTransport, W: TWriteTransport>(
            &self,
            _: &mut FInputProtocol<R>,
            oprot: &mut FOutputProtocol<W>,
        ) -> thrift::Result<()> {
            let mut proxy = oprot.t_protocol_proxy();
            proxy.write_string("Hello, World!")?;
            proxy.flush()
        }
    }

    let request_bytes = "Hello from the other side".as_bytes();
    let framed_request_bytes = prepend_frame_size(request_bytes);

    let addr = new_test_address();

    // set up server in another thread
    let server_addr = addr.clone();
    task::spawn(async move {
        let iprot = FInputProtocolFactory::new(TCompactInputProtocolFactory::new());
        let oprot =
            frugal::protocol::FOutputProtocolFactory::new(TCompactOutputProtocolFactory::new());
        let mut app = tide::new();
        app.at("/").nest(server::new(MockProcessor, iprot, oprot));
        app.listen(server_addr).await
    });

    let transport = client::FHttpTransportBuilder::new(
        reqwest::Client::new(),
        format!("http://{}/frugal", addr).parse().unwrap(),
    )
    .with_request_headers({
        let mut request_headers = HeaderMap::new();
        request_headers.insert(FOO, HeaderValue::from_str("bar").unwrap());
        request_headers
    })
    .with_request_headers_from_fcontext(get_request_headers)
    .build();

    let fctx = FContext::new(None);

    match transport.request(&fctx, &[0u8, 0, 0, 0]).await {
        Err(err) => panic!(err),
        Ok(_) => (),
    };

    let mut str_from_proc = "".to_string();
    let out = transport
        .request(&fctx, &framed_request_bytes)
        .await
        .unwrap()
        .read_to_string(&mut str_from_proc)
        .unwrap();

    assert_eq!(18, out);

    // use index 5 to skip the protocol version byte and size u32
    assert_eq!("Hello, World!", &str_from_proc[5..]);
}
