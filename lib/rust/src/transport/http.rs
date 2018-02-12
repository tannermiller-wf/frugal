use std::error::Error;
use std::io::{self, Cursor};
use std::str::from_utf8 as str_from_utf8;
use std::sync::{Arc, Mutex};

use base64;
use byteorder::{BigEndian, WriteBytesExt};
use futures::future::{self, Future};
use futures::stream::Stream;
use hyper::{self, Body, Method, StatusCode, Uri};
use hyper::client::{Client, HttpConnector};
use hyper::header::{qitem, Accept, ContentLength, ContentType, Headers};
use hyper::server::{Request, Response, Service};
use mime;
use thrift;
use thrift::transport::{TBufferedReadTransportFactory, TBufferedWriteTransportFactory,
                        TReadTransport, TReadTransportFactory, TWriteTransportFactory};

use context::FContextImpl;
use processor::FProcessor;
use protocol::{FInputProtocolFactory, FOutputProtocolFactory};
use transport::FTransport;
use util::read_size;

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

header!{ (PayloadLimit, "x-frugal-payload-limit") => [i64] }
header!{ (ContentTransferEncodingHeader, "content-transfer-encoding") => [String] }

static BASE64_ENCODING: &'static str = "base64";

lazy_static! {
    static ref FRUGAL_CONTENT_TYPE: mime::Mime = "application/x-frugal".parse().unwrap();
}

pub struct FHttpTransportBuilder {
    client: Client<HttpConnector, Body>,
    url: Uri,
    request_size_limit: Option<usize>,
    response_size_limit: Option<usize>,
    request_headers: Option<Headers>,
    get_request_headers: Option<Box<Fn(&FContextImpl) -> Headers>>,
}

impl FHttpTransportBuilder {
    pub fn new(client: Client<HttpConnector, Body>, url: Uri) -> Self {
        FHttpTransportBuilder {
            client: client,
            url: url,
            request_size_limit: None,
            response_size_limit: None,
            request_headers: None,
            get_request_headers: None,
        }
    }

    pub fn with_request_size_limit(&mut self, request_size_limit: usize) -> &mut Self {
        self.request_size_limit = Some(request_size_limit);
        self
    }

    pub fn with_response_size_limit(&mut self, response_size_limit: usize) -> &mut Self {
        self.response_size_limit = Some(response_size_limit);
        self
    }

    pub fn with_request_headers(&mut self, request_headers: Headers) -> &mut Self {
        self.request_headers = Some(request_headers);
        self
    }

    pub fn with_request_headers_from_fcontext(
        &mut self,
        get_request_headers: Box<Fn(&FContextImpl) -> Headers>,
    ) -> &mut Self {
        self.get_request_headers = Some(get_request_headers);
        self
    }

    pub fn build(self) -> FHttpTransport {
        FHttpTransport {
            client: self.client,
            url: self.url,
            request_size_limit: self.request_size_limit,
            response_size_limit: self.response_size_limit,
            request_headers: self.request_headers,
            get_request_headers: self.get_request_headers,
        }
    }
}

pub struct FHttpTransport {
    client: Client<HttpConnector, Body>,
    url: Uri,
    request_size_limit: Option<usize>,
    response_size_limit: Option<usize>,
    request_headers: Option<Headers>,
    get_request_headers: Option<Box<Fn(&FContextImpl) -> Headers>>,
}

impl FTransport for FHttpTransport {
    fn oneway(&self, ctx: &FContextImpl, payload: &[u8]) -> Option<thrift::Result<()>> {
        self.request(ctx, payload).map(|x| x.map(|_| ()))
    }

    fn request(
        &self,
        ctx: &FContextImpl,
        payload: &[u8],
    ) -> Option<thrift::Result<Box<TReadTransport>>> {
        // TODO: isOpen check goes here if needed

        // TODO: in the Go Request, there is a check for payload == and returns nil, nil. WTF is
        // that? This case is totally ignored in the Java code
        if payload.len() == 4 {
            return None;
        }

        if let Some(size) = self.request_size_limit {
            if payload.len() > size {
                return Some(Err(thrift::new_transport_error(
                    thrift::TransportErrorKind::SizeLimit,
                    format!(
                        "Message exceeds {} bytes, was {} bytes",
                        size,
                        payload.len()
                    ),
                )));
            }
        }

        // Encode request payload
        let mut encoded = Vec::new();
        encoded.append(&mut base64::encode(payload).into_bytes());

        // Initialize request
        let mut request: Request<Body> = Request::new(Method::Post, self.url.clone());
        request.set_body(encoded);

        // TODO: Hyper does not currently support timeouts, there is a hyper-timeout-connector that
        // does support timeouts, but at the Client and not the Request level.

        // add user supplied headers first, to avoid monkeying
        // with the size limits headers below.
        // add dynamic headers from fcontext first
        if let Some(ref get_request_headers) = self.get_request_headers {
            request
                .headers_mut()
                .extend(get_request_headers(ctx).iter())
        }

        // now add manually passed in request headers
        if let Some(ref headers) = self.request_headers {
            request.headers_mut().extend(headers.iter());
        }

        // Add request headers
        request
            .headers_mut()
            .set(ContentType(FRUGAL_CONTENT_TYPE.clone()));
        request
            .headers_mut()
            .set(Accept(vec![qitem(FRUGAL_CONTENT_TYPE.clone())]));
        request
            .headers_mut()
            .set(ContentTransferEncodingHeader(BASE64_ENCODING.to_string()));
        if let Some(size) = self.response_size_limit {
            request.headers_mut().set(PayloadLimit(size as i64));
        }

        // Make the HTTP request
        let body_result = self.client
            .request(request)
            .map_err(|err| thrift::Error::User(Box::new(err)))
            .and_then(|response| {
                if let StatusCode::PayloadTooLarge = response.status() {
                    return Err(thrift::new_transport_error(
                        thrift::TransportErrorKind::SizeLimit,
                        "response was too large for the transport",
                    ));
                }

                let status_code = response.status().as_u16();

                response
                    .body()
                    .collect()
                    .map_err(|err| thrift::Error::User(Box::new(err)))
                    .and_then(move |chunks| {
                        let mut buf = Vec::new();
                        for chunk in chunks {
                            buf.append(&mut chunk.to_vec());
                        }

                        {
                            let buf_str = match str_from_utf8(&buf) {
                                Ok(s) => s,
                                // NOTE: thrift::Error really should implement From<Utf8Error> too
                                Err(err) => return Err(thrift::Error::User(Box::new(err))),
                            };

                            if status_code >= 300 {
                                return Err(thrift::new_transport_error(
                                    thrift::TransportErrorKind::Unknown,
                                    format!(
                                        "response errored with code {} and message {}",
                                        status_code, buf_str
                                    ),
                                ));
                            }
                        }

                        base64::decode(&buf).map_err(|err| thrift::Error::User(Box::new(err)))
                    })
                    .wait()
            })
            .wait();

        if let Err(err) = body_result {
            return Some(Err(err));
        };
        let body = body_result.unwrap(); // Safe since we just returned if this was an Err

        let body_len = body.len();

        // All responses should be framed with 4 bytes (uint32)
        if body_len < 4 {
            return Some(Err(thrift::new_protocol_error(
                thrift::ProtocolErrorKind::InvalidData,
                "frugal: invalid invalid frame size",
            )));
        }

        let mut body_cursor = Cursor::new(body);

        // If there are only 4 bytes, this needs to be a one-way (i.e. frame size 0)
        if body_len == 4 {
            if let Ok(n) = read_size(&mut body_cursor) {
                if n != 0 {
                    return Some(Err(thrift::new_protocol_error(
                        thrift::ProtocolErrorKind::InvalidData,
                        "frugal: missing data",
                    )));
                }
            }
            // it's a one-way, drop it
            return None;
        }

        Some(Ok(TBufferedReadTransportFactory::new()
            .create(Box::new(body_cursor))))
    }

    fn get_request_size_limit(&self) -> Option<usize> {
        self.request_size_limit
    }
}

fn error_resp(err_msg: String) -> Response {
    error_resp_with_status(err_msg, hyper::StatusCode::BadRequest)
}

fn error_resp_with_status(err_msg: String, status: hyper::StatusCode) -> Response {
    Response::new()
        .with_status(status)
        .with_header(ContentLength(err_msg.len() as u64))
        .with_body(err_msg)
}

pub struct FHTTPService {
    processor: Arc<FProcessor>,
    input_protocol_factory: Arc<FInputProtocolFactory>,
    output_protocol_factory: Arc<FOutputProtocolFactory>,
}

impl FHTTPService {
    pub fn new(
        processor: Arc<FProcessor>,
        input_protocol_factory: Arc<FInputProtocolFactory>,
        output_protocol_factory: Arc<FOutputProtocolFactory>,
    ) -> Self {
        FHTTPService {
            processor,
            input_protocol_factory,
            output_protocol_factory,
        }
    }
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
                    hyper::StatusCode::InternalServerError,
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
                        hyper::StatusCode::PayloadTooLarge,
                    )));
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

#[cfg(test)]
mod test {
    use std::fmt;

    use thrift;

    use context::{FContext, FContextImpl};
    use protocol::{FInputProtocol, FOutputProtocol};
    use super::*;

    fn response_to_string(response: hyper::Response) -> String {
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
        let service = FHTTPService::new(Arc::new(MockProcessor), in_prot_fac, out_prot_fac);

        let request: Request<hyper::Body> =
            hyper::Request::new(hyper::Method::Post, "http://example.com".parse().unwrap());
        let response = service.call(request).wait().unwrap();

        assert_eq!(hyper::StatusCode::BadRequest, response.status());
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
        let service = FHTTPService::new(Arc::new(MockProcessor), in_prot_fac, out_prot_fac);

        let mut request: Request<hyper::Body> =
            hyper::Request::new(hyper::Method::Post, "http://example.com".parse().unwrap());
        request.set_body(base64::encode(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8]));
        request.headers_mut().set(ContentLength(9));
        let response = service.call(request).wait().unwrap();

        assert_eq!(hyper::StatusCode::InternalServerError, response.status());
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
                let mut ctx = FContextImpl::new(None);
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
        let service = FHTTPService::new(Arc::new(MockProcessor), in_prot_fac, out_prot_fac);

        let mut request: Request<hyper::Body> =
            hyper::Request::new(hyper::Method::Post, "http://example.com".parse().unwrap());
        request.set_body(base64::encode(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8]));
        request.headers_mut().set(ContentLength(9));
        request.headers_mut().set(PayloadLimit(5));
        let response = service.call(request).wait().unwrap();

        assert_eq!(hyper::StatusCode::PayloadTooLarge, response.status());
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
                let mut ctx = FContextImpl::new(None);
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
        let service = FHTTPService::new(Arc::new(MockProcessor), in_prot_fac, out_prot_fac);

        let mut request: Request<hyper::Body> =
            hyper::Request::new(hyper::Method::Post, "http://example.com".parse().unwrap());
        request.set_body(base64::encode(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8]));
        request.headers_mut().set(ContentLength(9));
        let response = service.call(request).wait().unwrap();
        assert_eq!(hyper::StatusCode::Ok, response.status());
        let out = response_to_string(response);

        let mut iprot = FInputProtocolFactory::new(Box::new(
            thrift::protocol::TCompactInputProtocolFactory::new(),
        )).get_protocol(Box::new(Cursor::new(base64::decode(&out[4..]).unwrap())));
        let mut out_ctx = FContextImpl::new(None);
        iprot.read_response_header(&mut out_ctx).unwrap();
        assert_eq!("bar", out_ctx.response_header("foo").unwrap());
    }

    // TODO: Tests with stood up http server
}
