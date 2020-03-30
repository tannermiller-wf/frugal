use std::io::Cursor;
use std::str::from_utf8 as str_from_utf8;

use async_trait::async_trait;
use base64;
use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{Client, StatusCode, Url};
use thrift::transport::{TBufferedReadTransportFactory, TReadTransport, TReadTransportFactory};

use super::{BASE64_ENCODING, CONTENT_TRANSFER_ENCODING, FRUGAL_CONTENT_TYPE, PAYLOAD_LIMIT};
use crate::context::FContext;
use crate::transport::FTransport;
use crate::util::read_size;

pub struct FHttpTransportBuilder {
    client: Client,
    url: Url,
    request_size_limit: Option<usize>,
    response_size_limit: Option<usize>,
    request_headers: Option<HeaderMap>,
    get_request_headers: Option<fn(&FContext) -> HeaderMap>,
}

impl FHttpTransportBuilder {
    pub fn new(client: Client, url: Url) -> Self {
        FHttpTransportBuilder {
            client,
            url,
            request_size_limit: None,
            response_size_limit: None,
            request_headers: None,
            get_request_headers: None,
        }
    }

    pub fn with_request_size_limit(mut self, request_size_limit: usize) -> Self {
        self.request_size_limit = Some(request_size_limit);
        self
    }

    pub fn with_response_size_limit(mut self, response_size_limit: usize) -> Self {
        self.response_size_limit = Some(response_size_limit);
        self
    }

    pub fn with_request_headers(mut self, request_headers: HeaderMap) -> Self {
        self.request_headers = Some(request_headers);
        self
    }

    pub fn with_request_headers_from_fcontext(
        mut self,
        get_request_headers: fn(&FContext) -> HeaderMap,
    ) -> Self {
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

#[derive(Clone)]
pub struct FHttpTransport {
    client: Client,
    url: Url,
    request_size_limit: Option<usize>,
    response_size_limit: Option<usize>,
    request_headers: Option<HeaderMap>,
    get_request_headers: Option<fn(&FContext) -> HeaderMap>,
}

#[async_trait]
impl FTransport for FHttpTransport {
    type Response = Box<dyn TReadTransport + Send>;

    async fn oneway(&self, ctx: &FContext, payload: &[u8]) -> thrift::Result<()> {
        self.request(ctx, payload).await.map(|_| ())
    }

    async fn request(&self, ctx: &FContext, payload: &[u8]) -> thrift::Result<Self::Response> {
        // TODO: isOpen check goes here if needed

        if payload.len() == 4 {
            return Ok(Box::new(Cursor::new(Bytes::new()))); // TODO: WTF should I return here?
        };

        if let Some(size) = self.request_size_limit {
            if payload.len() > size {
                return Err(thrift::new_transport_error(
                    thrift::TransportErrorKind::SizeLimit,
                    format!(
                        "Message exceeds {} bytes, was {} bytes",
                        size,
                        payload.len()
                    ),
                ));
            }
        };

        // Encode request payload
        let mut encoded = Vec::new();
        encoded.append(&mut base64::encode(payload).into_bytes());

        // set headers
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_LENGTH, encoded.len().into());
        headers.insert(CONTENT_TYPE, HeaderValue::from_static(FRUGAL_CONTENT_TYPE));
        headers.insert(ACCEPT, HeaderValue::from_static(FRUGAL_CONTENT_TYPE));
        headers.insert(
            CONTENT_TRANSFER_ENCODING,
            HeaderValue::from_static(BASE64_ENCODING),
        );
        if let Some(ref get_request_headers) = self.get_request_headers {
            headers.extend(get_request_headers(ctx));
        };
        if let Some(size) = self.response_size_limit {
            headers.insert(PAYLOAD_LIMIT, size.into());
        }

        let result = self
            .client
            .post(self.url.clone())
            .headers(headers)
            .timeout(ctx.timeout())
            .body(encoded)
            .send()
            .await;

        let response = match result {
            Ok(response) => response,
            Err(err) => {
                if err.is_timeout() {
                    return Err(thrift::new_transport_error(
                        thrift::TransportErrorKind::TimedOut,
                        "timed out",
                    ));
                } else {
                    return Err(thrift::Error::User(Box::new(err)));
                };
            }
        };

        let status_code = response.status();
        if status_code == StatusCode::PAYLOAD_TOO_LARGE {
            return Err(thrift::new_transport_error(
                thrift::TransportErrorKind::SizeLimit,
                "response was too large for the transport",
            ));
        };

        let body = match response.bytes().await {
            Ok(body) => body,
            Err(err) => return Err(thrift::Error::User(Box::new(err))),
        };

        if status_code.as_u16() >= 300 {
            return match str_from_utf8(&body) {
                Ok(s) => Err(thrift::new_transport_error(
                    thrift::TransportErrorKind::Unknown,
                    format!(
                        "response errored with code {} and message {}",
                        status_code, s
                    ),
                )),
                Err(err) => Err(thrift::Error::User(Box::new(err))),
            };
        }

        let decoded_body = match base64::decode(&body) {
            Ok(body) => body,
            Err(err) => return Err(thrift::Error::User(Box::new(err))),
        };

        let body_len = decoded_body.len();

        // All responses should be framed with 4 bytes (uint32)
        if body_len < 4 {
            return Err(thrift::new_protocol_error(
                thrift::ProtocolErrorKind::InvalidData,
                "frugal: invalid invalid frame size",
            ));
        }

        let mut body_cursor = Cursor::new(decoded_body);

        // If there are only 4 bytes, this needs to be a one-way (i.e. frame size 0)
        if body_len == 4 {
            if let Ok(n) = read_size(&mut body_cursor) {
                if n != 0 {
                    return Err(thrift::new_protocol_error(
                        thrift::ProtocolErrorKind::InvalidData,
                        "frugal: missing data",
                    ));
                }
            }
            // it's a one-way, drop it
            return Ok(Box::new(Cursor::new(Bytes::new())));
        }

        Ok(TBufferedReadTransportFactory::new().create(Box::new(body_cursor)))
    }

    fn get_request_size_limit(&self) -> Option<usize> {
        self.request_size_limit
    }
}
