use std::error::Error;
use std::io::{self, Cursor};
use std::sync::{Arc, Mutex};

use base64;
use byteorder::{BigEndian, WriteBytesExt};
use futures::future::{self, Future};
use futures::stream::Stream;
use hyper;
use hyper::header::{ContentLength, ContentType};
use hyper::server::{Request, Response, Service};
use mime;
use thrift::transport::{TBufferedReadTransportFactory, TBufferedWriteTransportFactory,
                        TReadTransport, TReadTransportFactory, TWriteTransport,
                        TWriteTransportFactory};
use thrift::protocol::{TInputProtocolFactory, TOutputProtocolFactory};
use thrift::server::TProcessor;

// TODO: Figure out what we need for FProcessor and kin
//use processor::FProcessor;
//use protocol::{FInputProtocolFactory, FOutputProtocolFactory};

// TODO: could this be an RC and RefCell?
struct MutexWriteTransport(Arc<Mutex<Vec<u8>>>);

impl io::Write for MutexWriteTransport {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

header!{ (PayloadLimitHeader, "x-frugal-payload-limit") => [i64] }
header!{ (ContentTransferEncodingHeader, "content-transfer-encoding") => [String] }

static BASE64_ENCODING: &'static str = "base64";

lazy_static! {
    static ref FRUGAL_CONTENT_TYPE: mime::Mime = "application/x-frugal".parse().unwrap();
}

pub trait FReadTransport: TReadTransport {}

pub trait FWriteTransport: TWriteTransport {}

pub struct FHTTPService {
    processor: Arc<TProcessor>,
    input_protocol_factory: Arc<TInputProtocolFactory>,
    output_protocol_factory: Arc<TOutputProtocolFactory>,
}

impl FHTTPService {
    pub fn new(
        processor: Arc<TProcessor>,
        input_protocol_factory: Arc<TInputProtocolFactory>,
        output_protocol_factory: Arc<TOutputProtocolFactory>,
    ) -> Self {
        FHTTPService {
            processor,
            input_protocol_factory,
            output_protocol_factory,
        }
    }
}

fn error_resp(err_msg: String) -> Response {
    Response::new()
        .with_status(hyper::StatusCode::BadRequest)
        .with_header(ContentLength(err_msg.len() as u64))
        .with_body(err_msg)
}

impl Service for FHTTPService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        // validate we have at least enough for a frame header
        let req_len = match req.headers().get::<ContentLength>() {
            Some(&ContentLength(len)) => len,
            None => 0,
        };
        if req_len < 4 {
            return Box::new(future::ok(error_resp(format!(
                "Invalid request size {}",
                req_len
            ))));
        };

        // pull out the payload limit header
        let limit = req.headers()
            .get::<PayloadLimitHeader>()
            .map(|&PayloadLimitHeader(val)| val);

        // clone these for use in the future
        let processor = self.processor.clone();
        let input_protocol_factory = Arc::clone(&self.input_protocol_factory);
        let output_protocol_factory = Arc::clone(&self.output_protocol_factory);

        // box up the future for this request
        Box::new(req.body().collect().and_then(move |chunks| {
            // pull all the chunks and move them into a Vec<u8>
            let mut buf = Vec::new();
            for chunk in chunks {
                buf.append(&mut chunk.to_vec());
            }

            // decode the body
            let decoded = match base64::decode(&buf) {
                Ok(decoded) => decoded,
                Err(err) => {
                    return Box::new(future::ok(error_resp(format!(
                        "Error processing request: {}",
                        err.description()
                    ))));
                }
            };

            // get input transport and protocol
            let read_fac = TBufferedReadTransportFactory::new();
            let mut cursor = Cursor::new(decoded);
            cursor.set_position(4);
            let input = read_fac.create(Box::new(cursor));
            let mut iprot = input_protocol_factory.create(input);

            // get output transport and protocol
            let write_fac = TBufferedWriteTransportFactory::new();
            let out_buf = Arc::new(Mutex::new(Vec::new()));
            let output = write_fac.create(Box::new(MutexWriteTransport(Arc::clone(&out_buf))));
            let mut oprot = output_protocol_factory.create(output);

            // run processor
            if let Err(err) = processor.process(&mut *iprot, &mut *oprot) {
                return Box::new(future::ok(error_resp(format!(
                    "Error processing request: {}",
                    err.description()
                ))));
            };

            // get len in a block to release the lock as soon as it's done
            let out_buf_len = { out_buf.lock().unwrap().len() };

            // enforce limit, if provided
            if let Some(limit) = limit {
                if out_buf_len > limit as usize {
                    return Box::new(future::ok(error_resp(format!(
                        "Response size ({}) larger than requested size({})",
                        out_buf_len, limit
                    ))));
                }
            };

            let mut encoded = Vec::new();

            // encode the length as the initial 4 bytes (frame size)
            if let Err(err) = encoded.write_u32::<BigEndian>(out_buf_len as u32) {
                return Box::new(future::err(err.into()));
            };

            // encode as base64
            encoded.append(&mut base64::encode(&*out_buf.lock().unwrap()).into_bytes());

            // write to response
            Box::new(future::ok(
                Response::new()
                    .with_header(ContentType(FRUGAL_CONTENT_TYPE.clone()))
                    .with_header(ContentLength(encoded.len() as u64))
                    .with_header(ContentTransferEncodingHeader(BASE64_ENCODING.to_string()))
                    .with_body(encoded),
            ))
        }))
    }
}
