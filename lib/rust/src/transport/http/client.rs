use std::io::Cursor;
use std::str::from_utf8 as str_from_utf8;

use base64;
use futures::future::{Either, Future};
use futures::stream::Stream;
use hyper::{Body, Method, StatusCode, Uri};
use hyper::client::{Client, HttpConnector};
use hyper::header::{qitem, Accept, ContentLength, ContentType, Headers};
use hyper::server::Request;
use thrift;
use thrift::transport::{TBufferedReadTransportFactory, TReadTransport, TReadTransportFactory};
use tokio_core::reactor::{Core, Timeout};

use context::FContext;
use transport::FTransport;
use util::read_size;
use super::*;

pub struct FHttpTransportBuilder {
    core: Core,
    client: Client<HttpConnector, Body>,
    url: Uri,
    request_size_limit: Option<usize>,
    response_size_limit: Option<usize>,
    request_headers: Option<Headers>,
    get_request_headers: Option<Box<Fn(&FContext) -> Headers>>,
}

impl FHttpTransportBuilder {
    pub fn new(client: Client<HttpConnector, Body>, core: Core, url: Uri) -> Self {
        FHttpTransportBuilder {
            core: core,
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
        get_request_headers: Box<Fn(&FContext) -> Headers>,
    ) -> &mut Self {
        self.get_request_headers = Some(get_request_headers);
        self
    }

    pub fn build(self) -> FHttpTransport {
        FHttpTransport {
            core: self.core,
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
    core: Core,
    client: Client<HttpConnector, Body>,
    url: Uri,
    request_size_limit: Option<usize>,
    response_size_limit: Option<usize>,
    request_headers: Option<Headers>,
    get_request_headers: Option<Box<Fn(&FContext) -> Headers>>,
}

impl FTransport for FHttpTransport {
    fn oneway(&mut self, ctx: &FContext, payload: &[u8]) -> Option<thrift::Result<()>> {
        self.request(ctx, payload).map(|x| x.map(|_| ()))
    }

    fn request(
        &mut self,
        ctx: &FContext,
        payload: &[u8],
    ) -> Option<thrift::Result<Box<TReadTransport>>> {
        // TODO: isOpen check goes here if needed

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
        request
            .headers_mut()
            .set(ContentLength(encoded.len() as u64));
        request.set_body(encoded);

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

        let timeout = match Timeout::new(ctx.timeout(), &self.core.handle()) {
            Ok(to) => to,
            Err(err) => {
                return Some(Err(thrift::new_transport_error(
                    thrift::TransportErrorKind::Unknown,
                    format!("setting timeout error: {}", &err),
                )))
            }
        };

        // Make the HTTP request
        let initial_request = self.client
            .request(request)
            .map_err(|err| thrift::Error::User(Box::new(err)))
            .and_then(|response| {
                if let StatusCode::PayloadTooLarge = response.status() {
                    return Err(thrift::new_transport_error(
                        thrift::TransportErrorKind::SizeLimit,
                        "response was too large for the transport",
                    ));
                }

                Ok(response)
            });

        // Handle timeout
        let op = initial_request
            .select2(timeout)
            .then(|res| match res {
                // TODO: Can this be simplified with Either::split?
                Ok(Either::A((resp, _))) => Ok(resp),
                Ok(Either::B((_, _to))) => Err(thrift::new_transport_error(
                    thrift::TransportErrorKind::TimedOut,
                    "timed out",
                )),
                Err(Either::A((resp_err, _))) => Err(resp_err),
                Err(Either::B((_, _to_err))) => Err(thrift::new_transport_error(
                    thrift::TransportErrorKind::TimedOut,
                    "time out error",
                )),
            })
            .and_then(|resp| {
                let status_code = resp.status().as_u16();

                resp.body()
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
            });

        let body = match self.core.run(op) {
            Ok(body) => body,
            Err(err) => return Some(Err(err)),
        };

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
