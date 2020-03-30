use std::io::Cursor;

use base64;
use byteorder::{BigEndian, WriteBytesExt};
use http::header::HeaderMap;
use http::status::StatusCode;
use thrift::transport::{TBufferedReadTransportFactory, TReadTransportFactory};
use tide::{self, Request, Response, Server};

use super::{
    BASE64_ENCODING, CONTENT_LENGTH, CONTENT_TRANSFER_ENCODING, CONTENT_TYPE, FRUGAL_CONTENT_TYPE,
    PAYLOAD_LIMIT,
};
use crate::processor::FProcessor;
use crate::protocol::{FInputProtocolFactory, FOutputProtocolFactory};

fn error_resp(err_msg: String, status: StatusCode) -> Response {
    Response::new(status.as_u16())
        .set_header(CONTENT_LENGTH.as_str(), err_msg.len().to_string())
        .body_string(err_msg)
}

pub struct State<P: FProcessor> {
    processor: P,
    input_protocol_factory: FInputProtocolFactory,
    output_protocol_factory: FOutputProtocolFactory,
}

/// Build a tide::Server that serves at "/frugal".
pub fn new<P: FProcessor>(
    processor: P,
    input_protocol_factory: FInputProtocolFactory,
    output_protocol_factory: FOutputProtocolFactory,
) -> Server<State<P>> {
    let mut server = tide::with_state(State {
        processor,
        input_protocol_factory,
        output_protocol_factory,
    });
    server
        .at("/frugal") // TODO: Perhaps this out to just be "/" and or make this configurable
        .post(|mut req: Request<State<P>>| async move {
            let body = match req.body_bytes().await {
                Ok(body) => body,
                Err(err) => {
                    return error_resp(
                        format!("Could not read the frugal frame bytes: {}", err),
                        StatusCode::BAD_REQUEST,
                    );
                }
            };
            match process(req.headers(), req.state(), &body) {
                Ok(resp) => Response::new(StatusCode::OK.as_u16())
                    .set_header(CONTENT_TYPE.as_ref(), &*FRUGAL_CONTENT_TYPE)
                    .set_header(CONTENT_LENGTH.as_ref(), resp.len().to_string())
                    .set_header(CONTENT_TRANSFER_ENCODING, &*BASE64_ENCODING)
                    .body_string(resp),
                Err((err_msg, status)) => error_resp(err_msg, status),
            }
        });
    server
}

fn process<P: FProcessor>(
    headers: &HeaderMap,
    state: &State<P>,
    body: &[u8],
) -> Result<String, (String, StatusCode)> {
    // validate we have at least enough for a frame header
    let req_len = headers
        .get(&CONTENT_LENGTH)
        .and_then(|h| h.to_str().unwrap().parse::<i64>().ok())
        .unwrap_or(0);
    if req_len < 4 {
        return Err((
            format!("Invalid request size {}", req_len),
            StatusCode::BAD_REQUEST,
        ));
    }

    // pull out the payload limit header
    let limit = headers
        .get(&*PAYLOAD_LIMIT)
        .and_then(|h| h.to_str().unwrap().parse::<i64>().ok());

    let decoded_body = match base64::decode(&body) {
        Ok(decoded) => decoded,
        Err(err) => {
            return Err((
                format!("Error processing request: {}", err),
                StatusCode::BAD_REQUEST,
            ))
        }
    };

    let mut cursor = Cursor::new(decoded_body);
    cursor.set_position(4);

    // get input transport and protocol
    let read_fac = TBufferedReadTransportFactory::new();
    let input = read_fac.create(Box::new(cursor));
    let mut iprot = state.input_protocol_factory.get_protocol(input);

    // get output transport and protocol
    //let write_fac = TBufferedWriteTransportFactory::new();
    let mut oprot = state.output_protocol_factory.get_protocol(Vec::new());

    // run processor
    if let Err(err) = state.processor.process(&mut iprot, &mut oprot) {
        return Err((
            format!("Error processing request: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        ));
    };

    // get len in a block to release the lock as soon as it's done
    let out_buf = oprot.into_transport();
    let out_buf_len = out_buf.len();

    // enforce limit, if provided
    if let Some(limit) = limit {
        if out_buf_len > limit as usize {
            return Err((
                format!(
                    "Response size ({}) larger than requested size({})",
                    out_buf_len, limit
                ),
                StatusCode::PAYLOAD_TOO_LARGE,
            ));
        }
    };

    // encode the length as the initial 4 bytes (frame size)
    // NOTE: no I/O is happening so no error should be possible.
    let mut output_buf = Vec::new();
    output_buf
        .write_u32::<BigEndian>(out_buf_len as u32)
        .unwrap();

    output_buf.extend_from_slice(&out_buf);
    Ok(base64::encode(&output_buf))
}

#[cfg(test)]
mod test {
    use std::error::Error;
    use std::fmt;

    use http::header::{HeaderMap, HeaderValue};
    use http::status::StatusCode;
    use thrift;
    use thrift::protocol::TOutputProtocol;
    use thrift::transport::{TReadTransport, TWriteTransport};

    use super::*;
    use crate::context::FContext;
    use crate::protocol::{FInputProtocol, FOutputProtocol};

    #[test]
    fn test_process_bad_frame_error() {
        #[derive(Clone)]
        struct MockProcessor;
        impl FProcessor for MockProcessor {
            fn process<R: TReadTransport, W: TWriteTransport>(
                &self,
                _: &mut FInputProtocol<R>,
                _: &mut FOutputProtocol<W>,
            ) -> thrift::Result<()> {
                Ok(())
            }
        }

        let in_prot_fac =
            FInputProtocolFactory::new(thrift::protocol::TCompactInputProtocolFactory::new());
        let out_prot_fac =
            FOutputProtocolFactory::new(thrift::protocol::TCompactOutputProtocolFactory::new());
        let state = State {
            processor: MockProcessor,
            input_protocol_factory: in_prot_fac,
            output_protocol_factory: out_prot_fac,
        };
        let headers = HeaderMap::new();

        let (err_msg, status) = process(&headers, &state, &vec![]).unwrap_err();

        assert_eq!(StatusCode::BAD_REQUEST, status);
        assert_eq!("Invalid request size 0", err_msg);
    }

    #[test]
    fn test_process_processor_error() {
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
        #[derive(Clone)]
        struct MockProcessor;
        impl FProcessor for MockProcessor {
            fn process<R: TReadTransport, W: TWriteTransport>(
                &self,
                _: &mut FInputProtocol<R>,
                _: &mut FOutputProtocol<W>,
            ) -> thrift::Result<()> {
                // TODO: validate body is passed in here correctly
                Err(thrift::Error::User(Box::new(MockError)))
            }
        }

        let in_prot_fac =
            FInputProtocolFactory::new(thrift::protocol::TCompactInputProtocolFactory::new());
        let out_prot_fac =
            FOutputProtocolFactory::new(thrift::protocol::TCompactOutputProtocolFactory::new());
        let state = State {
            processor: MockProcessor,
            input_protocol_factory: in_prot_fac,
            output_protocol_factory: out_prot_fac,
        };

        let body = base64::encode(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8]);
        let mut headers = HeaderMap::new();
        headers.insert(&CONTENT_LENGTH, HeaderValue::from_str("9").unwrap());

        let (err_msg, status) = process(&headers, &state, body.as_bytes()).unwrap_err();

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, status);
        assert_eq!("Error processing request: processor error", err_msg,);
    }

    #[test]
    fn test_process_payload_too_large_error() {
        #[derive(Clone)]
        struct MockProcessor;
        impl FProcessor for MockProcessor {
            fn process<R: TReadTransport, W: TWriteTransport>(
                &self,
                _: &mut FInputProtocol<R>,
                oprot: &mut FOutputProtocol<W>,
            ) -> thrift::Result<()> {
                // TODO: validate body is passed in here correctly
                let mut ctx = FContext::new(None);
                ctx.add_response_header("foo", "bar");
                oprot.write_response_header(&mut ctx)
            }
        }

        let in_prot_fac =
            FInputProtocolFactory::new(thrift::protocol::TCompactInputProtocolFactory::new());
        let out_prot_fac =
            FOutputProtocolFactory::new(thrift::protocol::TCompactOutputProtocolFactory::new());
        let state = State {
            processor: MockProcessor,
            input_protocol_factory: in_prot_fac,
            output_protocol_factory: out_prot_fac,
        };

        let body = base64::encode(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8]);
        let mut headers = HeaderMap::new();
        headers.insert(&CONTENT_LENGTH, HeaderValue::from_str("9").unwrap());
        headers.insert(PAYLOAD_LIMIT, HeaderValue::from_str("5").unwrap());

        let (err_msg, status) = process(&headers, &state, body.as_bytes()).unwrap_err();

        assert_eq!(StatusCode::PAYLOAD_TOO_LARGE, status);
        assert_eq!("Response size (19) larger than requested size(5)", err_msg,);
    }

    #[test]
    fn test_process_happy_path() {
        #[derive(Clone)]
        struct MockProcessor;
        impl FProcessor for MockProcessor {
            fn process<R: TReadTransport, W: TWriteTransport>(
                &self,
                _: &mut FInputProtocol<R>,
                oprot: &mut FOutputProtocol<W>,
            ) -> thrift::Result<()> {
                // TODO: validate body is passed in here correctly
                let mut ctx = FContext::new(None);
                ctx.add_response_header("foo", "bar");
                oprot.write_response_header(&mut ctx)?;
                oprot.t_protocol_proxy().flush()
            }
        }

        let in_prot_fac =
            FInputProtocolFactory::new(thrift::protocol::TCompactInputProtocolFactory::new());
        let out_prot_fac =
            FOutputProtocolFactory::new(thrift::protocol::TCompactOutputProtocolFactory::new());
        let state = State {
            processor: MockProcessor,
            input_protocol_factory: in_prot_fac,
            output_protocol_factory: out_prot_fac,
        };

        let body = base64::encode(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8]);
        let mut headers = HeaderMap::new();
        headers.insert(&CONTENT_LENGTH, HeaderValue::from_str("9").unwrap());

        let resp = process(&headers, &state, body.as_bytes()).unwrap();

        let mut cursor = Cursor::new(base64::decode(&resp).unwrap());
        cursor.set_position(4);
        let mut iprot =
            FInputProtocolFactory::new(thrift::protocol::TCompactInputProtocolFactory::new())
                .get_protocol(Box::new(cursor));
        let mut out_ctx = FContext::new(None);
        iprot.read_response_header(&mut out_ctx).unwrap();
        assert_eq!("bar", out_ctx.response_header("foo").unwrap());
    }
}
