/*use std::error::Error;
use std::io::{self, Cursor};
use std::sync::{Arc, Mutex};

use base64;
use byteorder::{BigEndian, WriteBytesExt};
use futures::future::{self, Future};
use futures::stream::Stream;
use hyper::header::{ContentLength, ContentType};
use hyper::server::{NewService, Request, Response, Service};
use hyper::{self, Body, StatusCode};
use thrift::transport::{
    TBufferedReadTransportFactory, TBufferedWriteTransportFactory, TReadTransportFactory,
    TWriteTransportFactory,
};

use super::*;
use processor::FProcessor;
use protocol::{FInputProtocolFactory, FOutputProtocolFactory};

struct MutexWriteTransport(Arc<Mutex<Vec<u8>>>);

impl io::Write for MutexWriteTransport {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

fn error_resp(err_msg: String) -> Response {
    error_resp_with_status(err_msg, StatusCode::BadRequest)
}

fn error_resp_with_status(err_msg: String, status: StatusCode) -> Response {
    Response::new()
        .with_status(status)
        .with_header(ContentLength(err_msg.len() as u64))
        .with_body(err_msg)
}

pub struct FHttpService {
    processor: Arc<FProcessor>,
    input_protocol_factory: Arc<FInputProtocolFactory>,
    output_protocol_factory: Arc<FOutputProtocolFactory>,
}

impl FHttpService {
    pub fn new(
        processor: Arc<FProcessor>,
        input_protocol_factory: Arc<FInputProtocolFactory>,
        output_protocol_factory: Arc<FOutputProtocolFactory>,
    ) -> Self {
        FHttpService {
            processor,
            input_protocol_factory,
            output_protocol_factory,
        }
    }
}

impl NewService for FHttpService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Instance = Self;

    fn new_service(&self) -> Result<Self, io::Error> {
        Ok(self.clone())
    }
}

impl Service for FHttpService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error>>;

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
        let limit = req
            .headers()
            .get::<PayloadLimit>()
            .map(|&PayloadLimit(val)| val);

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
            let mut cursor = Cursor::new(decoded);
            cursor.set_position(4);

            // get input transport and protocol
            let read_fac = TBufferedReadTransportFactory::new();
            let input = read_fac.create(Box::new(cursor));
            let mut iprot = input_protocol_factory.get_protocol(input);

            // get output transport and protocol
            let write_fac = TBufferedWriteTransportFactory::new();
            let out_buf = Arc::new(Mutex::new(Vec::new()));
            let output = write_fac.create(Box::new(MutexWriteTransport(Arc::clone(&out_buf))));
            let mut oprot = output_protocol_factory.get_protocol(output);

            // run processor
            if let Err(err) = processor.process(&mut iprot, &mut oprot) {
                return Box::new(future::ok(error_resp_with_status(
                    format!("Error processing request: {}", err.description()),
                    StatusCode::InternalServerError,
                )));
            };

            // get len in a block to release the lock as soon as it's done
            let out_buf_len = { out_buf.lock().unwrap().len() };

            // enforce limit, if provided
            if let Some(limit) = limit {
                if out_buf_len > limit as usize {
                    return Box::new(future::ok(error_resp_with_status(
                        format!(
                            "Response size ({}) larger than requested size({})",
                            out_buf_len, limit
                        ),
                        StatusCode::PayloadTooLarge,
                    )));
                }
            };

            // encode the length as the initial 4 bytes (frame size)
            let mut output_buf = Vec::new();
            if let Err(err) = output_buf.write_u32::<BigEndian>(out_buf_len as u32) {
                return Box::new(future::err(err.into()));
            };

            // encode as base64
            output_buf.extend_from_slice(&*out_buf.lock().unwrap());
            let encoded = base64::encode(&output_buf).into_bytes();

            // write to response
            Box::new(future::ok(
                Response::<Body>::new()
                    .with_header(ContentType(FRUGAL_CONTENT_TYPE.clone()))
                    .with_header(ContentLength(encoded.len() as u64))
                    .with_header(ContentTransferEncodingHeader(BASE64_ENCODING.to_string()))
                    .with_body(encoded),
            ))
        }))
    }
}

impl Clone for FHttpService {
    fn clone(&self) -> Self {
        FHttpService {
            processor: self.processor.clone(),
            input_protocol_factory: self.input_protocol_factory.clone(),
            output_protocol_factory: self.output_protocol_factory.clone(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::fmt;

    use hyper::Method;
    use thrift;
    use thrift::protocol::TOutputProtocol;

    use super::*;
    use context::FContext;
    use protocol::{FInputProtocol, FOutputProtocol};

    fn response_to_string(response: Response) -> String {
        response
            .body()
            .collect()
            .and_then(move |chunks| {
                let mut v = Vec::new();
                for chunk in chunks {
                    v.append(&mut chunk.to_vec())
                }
                String::from_utf8(v).map_err(|err| hyper::Error::Utf8(err.utf8_error()))
            })
            .wait()
            .unwrap()
    }

    #[test]
    fn test_fhttp_service_bad_frame_error() {
        struct MockProcessor;
        impl FProcessor for MockProcessor {
            fn process(
                &self,
                _: &mut FInputProtocol,
                _: &mut FOutputProtocol,
            ) -> thrift::Result<()> {
                Ok(())
            }
        }

        let in_prot_fac = Arc::new(FInputProtocolFactory::new(Box::new(
            thrift::protocol::TCompactInputProtocolFactory::new(),
        )));
        let out_prot_fac = Arc::new(FOutputProtocolFactory::new(Box::new(
            thrift::protocol::TCompactOutputProtocolFactory::new(),
        )));
        let service = FHttpService::new(Arc::new(MockProcessor), in_prot_fac, out_prot_fac);

        let request: Request<Body> =
            Request::new(Method::Post, "http://example.com".parse().unwrap());
        let response = service.call(request).wait().unwrap();

        assert_eq!(StatusCode::BadRequest, response.status());
        assert_eq!("Invalid request size 0", response_to_string(response),);
    }

    #[test]
    fn test_fhttp_service_processor_error() {
        #[derive(Debug)]
        struct MockError;
        impl Error for MockError {
            fn description(&self) -> &str {
                "processor error"
            }
        }
        impl fmt::Display for MockError {
            fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                write!(f, "processor error")
            }
        }
        struct MockProcessor;
        impl FProcessor for MockProcessor {
            fn process(
                &self,
                _: &mut FInputProtocol,
                _: &mut FOutputProtocol,
            ) -> thrift::Result<()> {
                // TODO: validate body is passed in here correctly
                Err(thrift::Error::User(Box::new(MockError)))
            }
        }

        let in_prot_fac = Arc::new(FInputProtocolFactory::new(Box::new(
            thrift::protocol::TCompactInputProtocolFactory::new(),
        )));
        let out_prot_fac = Arc::new(FOutputProtocolFactory::new(Box::new(
            thrift::protocol::TCompactOutputProtocolFactory::new(),
        )));
        let service = FHttpService::new(Arc::new(MockProcessor), in_prot_fac, out_prot_fac);

        let mut request: Request<Body> =
            Request::new(Method::Post, "http://example.com".parse().unwrap());
        request.set_body(base64::encode(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8]));
        request.headers_mut().set(ContentLength(9));
        let response = service.call(request).wait().unwrap();

        assert_eq!(StatusCode::InternalServerError, response.status());
        assert_eq!(
            "Error processing request: processor error",
            response_to_string(response),
        );
    }

    #[test]
    fn test_fhttp_service_payload_too_large_error() {
        struct MockProcessor;
        impl FProcessor for MockProcessor {
            fn process(
                &self,
                _: &mut FInputProtocol,
                oprot: &mut FOutputProtocol,
            ) -> thrift::Result<()> {
                // TODO: validate body is passed in here correctly
                let mut ctx = FContext::new(None);
                ctx.add_response_header("foo", "bar");
                oprot.write_response_header(&mut ctx)
            }
        }

        let in_prot_fac = Arc::new(FInputProtocolFactory::new(Box::new(
            thrift::protocol::TCompactInputProtocolFactory::new(),
        )));
        let out_prot_fac = Arc::new(FOutputProtocolFactory::new(Box::new(
            thrift::protocol::TCompactOutputProtocolFactory::new(),
        )));
        let service = FHttpService::new(Arc::new(MockProcessor), in_prot_fac, out_prot_fac);

        let mut request: Request<Body> =
            Request::new(Method::Post, "http://example.com".parse().unwrap());
        request.set_body(base64::encode(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8]));
        request.headers_mut().set(ContentLength(9));
        request.headers_mut().set(PayloadLimit(5));
        let response = service.call(request).wait().unwrap();

        assert_eq!(StatusCode::PayloadTooLarge, response.status());
        assert_eq!(
            "Response size (19) larger than requested size(5)",
            response_to_string(response),
        );
    }

    #[test]
    fn test_fhttp_service_happy_path() {
        struct MockProcessor;
        impl FProcessor for MockProcessor {
            fn process(
                &self,
                _: &mut FInputProtocol,
                oprot: &mut FOutputProtocol,
            ) -> thrift::Result<()> {
                // TODO: validate body is passed in here correctly
                let mut ctx = FContext::new(None);
                ctx.add_response_header("foo", "bar");
                oprot.write_response_header(&mut ctx)?;
                oprot.flush()
            }
        }

        let in_prot_fac = Arc::new(FInputProtocolFactory::new(Box::new(
            thrift::protocol::TCompactInputProtocolFactory::new(),
        )));
        let out_prot_fac = Arc::new(FOutputProtocolFactory::new(Box::new(
            thrift::protocol::TCompactOutputProtocolFactory::new(),
        )));
        let service = FHttpService::new(Arc::new(MockProcessor), in_prot_fac, out_prot_fac);

        let mut request: Request<Body> =
            Request::new(Method::Post, "http://example.com".parse().unwrap());
        request.set_body(base64::encode(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8]));
        request.headers_mut().set(ContentLength(9));
        let response = service.call(request).wait().unwrap();
        assert_eq!(StatusCode::Ok, response.status());
        let out = response_to_string(response);

        let mut cursor = Cursor::new(base64::decode(&out).unwrap());
        cursor.set_position(4);
        let mut iprot = FInputProtocolFactory::new(Box::new(
            thrift::protocol::TCompactInputProtocolFactory::new(),
        ))
        .get_protocol(Box::new(cursor));
        let mut out_ctx = FContext::new(None);
        iprot.read_response_header(&mut out_ctx).unwrap();
        assert_eq!("bar", out_ctx.response_header("foo").unwrap());
    }
}*/
