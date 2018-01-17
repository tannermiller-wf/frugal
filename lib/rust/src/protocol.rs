use std::collections::BTreeMap;
use std::error::Error;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use byteorder::{BigEndian, WriteBytesExt};
use thrift;
use thrift::protocol::{TInputProtocol, TInputProtocolFactory, TOutputProtocol,
                       TOutputProtocolFactory};
use thrift::transport::{TReadTransport, TWriteTransport};

use context::{self, FContext, FContextImpl};
use util::{read_exact, read_size};

const PROTOCOL_V0: u8 = 0x00;

// This wraps the underlying transport in a thread safe way, so we can still access it after it
// is passed into the protocol factory.
struct TReadTransportWrapper(Arc<Mutex<Box<TReadTransport + Send>>>);

impl TReadTransportWrapper {
    fn wrap(tr_arc: &Arc<Mutex<Box<TReadTransport + Send>>>) -> Box<TReadTransport + Send> {
        Box::new(TReadTransportWrapper(Arc::clone(&tr_arc)))
    }
}

impl io::Read for TReadTransportWrapper {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

// This wraps the underlying transport in a thread safe way, so we can still access it after it
// is passed into the protocol factory.
struct TWriteTransportWrapper(Arc<Mutex<Box<TWriteTransport + Send>>>);

impl TWriteTransportWrapper {
    fn wrap(tr_arc: &Arc<Mutex<Box<TWriteTransport + Send>>>) -> Box<TWriteTransport + Send> {
        Box::new(TWriteTransportWrapper(Arc::clone(&tr_arc)))
    }
}

impl io::Write for TWriteTransportWrapper {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

enum ProtocolMarshaler {
    V0,
}

impl ProtocolMarshaler {
    fn get(version: u8) -> thrift::Result<ProtocolMarshaler> {
        match version {
            PROTOCOL_V0 => Ok(ProtocolMarshaler::V0),
            _ => Err(thrift::new_protocol_error(
                thrift::ProtocolErrorKind::BadVersion,
                format!("frugal: unsupported protocol version {}", version),
            )),
        }
    }

    fn unmarshal_headers<R: io::Read>(
        &self,
        reader: &mut R,
    ) -> thrift::Result<BTreeMap<String, String>> {
        match *self {
            ProtocolMarshaler::V0 => {
                let size = read_size(reader)?;

                let mut headers = BTreeMap::new();
                let mut i = 0;
                while i < size {
                    let name_size = read_size(reader)?;
                    i += 4;
                    if i > size || i + name_size > size {
                        return Err(thrift::new_protocol_error(
                            thrift::ProtocolErrorKind::InvalidData,
                            "frugal: invalid v0 protocol header name",
                        ));
                    }
                    let name = read_exact(reader, name_size).and_then(|bs| {
                        String::from_utf8(bs).map_err(|err| {
                            thrift::new_protocol_error(
                                thrift::ProtocolErrorKind::InvalidData,
                                err.description(),
                            )
                        })
                    })?;
                    i += name_size;

                    let value_size = read_size(reader)?;
                    i += 4;
                    if i > size || i + value_size > size {
                        return Err(thrift::new_protocol_error(
                            thrift::ProtocolErrorKind::InvalidData,
                            "frugal: invalid v0 protocol header value",
                        ));
                    }
                    let value = read_exact(reader, value_size).and_then(|bs| {
                        String::from_utf8(bs).map_err(|err| {
                            thrift::new_protocol_error(
                                thrift::ProtocolErrorKind::InvalidData,
                                err.description(),
                            )
                        })
                    })?;
                    i += value_size;

                    headers.insert(name, value);
                }

                Ok(headers)
            }
        }
    }

    fn marshal_headers(&self, headers: &BTreeMap<String, String>) -> thrift::Result<Vec<u8>> {
        match *self {
            ProtocolMarshaler::V0 => {
                let size = headers
                    .iter()
                    .fold(0, |size, (k, v)| size + 8 + k.len() + v.len());

                // Header buff = [version (1 byte), size (4 bytes), headers (size bytes)]
                // Headers = [size (4 bytes) name (size bytes) size (4 bytes) value (size bytes)*]
                let mut buf = Vec::with_capacity(size + 5);

                // Write version
                buf.push(PROTOCOL_V0);

                // Write size
                buf.write_u32::<BigEndian>(size as u32)?;

                // Write headers
                for (k, v) in headers.iter() {
                    buf.write_u32::<BigEndian>(k.len() as u32)?;
                    buf.write(k.as_bytes())?;
                    buf.write_u32::<BigEndian>(v.len() as u32)?;
                    buf.write(v.as_bytes())?;
                }

                Ok(buf)
            }
        }
    }
}

pub struct FInputProtocol {
    transport: Box<TReadTransport + Send>,
    protocol: Box<TInputProtocol + Send>,
}

// TODO: Should these deal with the FContext trait and not the impl?
//       This probably means waiting for the impl Trait feature to be stable
impl FInputProtocol {
    pub fn read_request_header(&mut self) -> thrift::Result<FContextImpl> {
        let headers = read_exact(&mut self.transport, 1)
            .and_then(|buf| ProtocolMarshaler::get(buf[0]))
            .and_then(|marshaler| marshaler.unmarshal_headers(&mut self.transport))?;

        let mut ctx = FContextImpl::new(None);
        for (k, v) in headers.iter() {
            // the new fcontext will have a new op id so don't copy the old one
            if k != context::OP_ID_HEADER {
                ctx.add_request_header(k.clone(), v.clone());
            }
        }

        let op_id = headers
            .get(context::OP_ID_HEADER)
            .ok_or(thrift::new_protocol_error(
                thrift::ProtocolErrorKind::InvalidData,
                "frugal: request missing op id",
            ))?;
        ctx.add_response_header(context::OP_ID_HEADER, op_id);

        if let Some(cid) = headers.get(context::CID_HEADER) {
            ctx.add_response_header(context::CID_HEADER, cid);
        }

        Ok(ctx)
    }

    pub fn read_response_header(&mut self, ctx: &mut FContextImpl) -> thrift::Result<()> {
        let headers = read_exact(&mut self.transport, 1)
            .and_then(|buf| ProtocolMarshaler::get(buf[0]))
            .and_then(|marshaler| marshaler.unmarshal_headers(&mut self.transport))?;

        for (k, v) in headers.iter() {
            if k != context::OP_ID_HEADER {
                ctx.add_response_header(k.clone(), v.clone());
            }
        }

        Ok(())
    }
}

pub struct FInputProtocolFactory {
    input_proto_factory: Box<TInputProtocolFactory>,
}

impl FInputProtocolFactory {
    pub fn new(input_proto_factory: Box<TInputProtocolFactory>) -> Self {
        FInputProtocolFactory {
            input_proto_factory,
        }
    }

    pub fn get_protocol(&self, tr: Box<TReadTransport + Send>) -> FInputProtocol {
        let tr_arc = Arc::new(Mutex::new(tr));

        FInputProtocol {
            transport: TReadTransportWrapper::wrap(&tr_arc),
            protocol: self.input_proto_factory
                .create(TReadTransportWrapper::wrap(&tr_arc)),
        }
    }
}

pub struct FOutputProtocol {
    transport: Box<TWriteTransport + Send>,
    protocol: Box<TOutputProtocol + Send>,
}

impl FOutputProtocol {
    fn write_header(&mut self, header: &BTreeMap<String, String>) -> thrift::Result<()> {
        ProtocolMarshaler::V0
            .marshal_headers(header)
            .and_then(|buf| {
                self.transport
                    .write(&buf)
                    .map_err(|err| {
                        thrift::new_transport_error(
                            thrift::TransportErrorKind::Unknown,
                            format!(
                                "frugal: error writing protocol headers in writeHeader: {}",
                                err.description()
                            ),
                        )
                    })
                    .and_then(|n| {
                        if n < buf.len() {
                            Err(thrift::new_transport_error(
                                thrift::TransportErrorKind::Unknown,
                                "frugal: failed to write complete protocol headers",
                            ))
                        } else {
                            Ok(())
                        }
                    })
            })
            .and_then(|_| {
                self.transport.flush().map_err(|err| {
                    thrift::new_transport_error(
                        thrift::TransportErrorKind::Unknown,
                        format!("frugal: failed to flush transport: {}", err.description()),
                    )
                })
            })
    }

    pub fn write_request_header(&mut self, ctx: &FContextImpl) -> thrift::Result<()> {
        self.write_header(ctx.request_headers())
    }

    pub fn write_response_header(&mut self, ctx: &FContextImpl) -> thrift::Result<()> {
        self.write_header(ctx.response_headers())
    }
}

pub struct FOutputProtocolFactory {
    output_proto_factory: Box<TOutputProtocolFactory>,
}

impl FOutputProtocolFactory {
    pub fn new(output_proto_factory: Box<TOutputProtocolFactory>) -> Self {
        FOutputProtocolFactory {
            output_proto_factory,
        }
    }

    pub fn get_protocol(&self, tr: Box<TWriteTransport + Send>) -> FOutputProtocol {
        let tr_arc = Arc::new(Mutex::new(tr));

        FOutputProtocol {
            transport: TWriteTransportWrapper::wrap(&tr_arc),
            protocol: self.output_proto_factory
                .create(TWriteTransportWrapper::wrap(&tr_arc)),
        }
    }
}

#[cfg(test)]
mod test {
    use std::fs;
    use std::io::{self, Cursor, Write};

    use thrift;
    use thrift::protocol::{TBinaryInputProtocolFactory, TBinaryOutputProtocolFactory};

    use super::*;
    use context::{FContext, FContextImpl, CID_HEADER, OP_ID_HEADER};

    static BASIC_FRAME: &'static [u8] = &[
        0, 0, 0, 0, 14, 0, 0, 0, 3, 102, 111, 111, 0, 0, 0, 3, 98, 97, 114
    ];
    static FRUGAL_FRAME: &'static [u8] = &[
        0, 0, 0, 0, 65, 0, 0, 0, 5, 104, 101, 108, 108, 111, 0, 0, 0, 5, 119, 111, 114, 108, 100,
        0, 0, 0, 5, 95, 111, 112, 105, 100, 0, 0, 0, 1, 48, 0, 0, 0, 4, 95, 99, 105, 100, 0, 0, 0,
        21, 105, 89, 65, 71, 67, 74, 72, 66, 87, 67, 75, 76, 74, 66, 115, 106, 107, 100, 111, 104,
        98,
    ];
    static FRUGAL_CID: &'static str = "iYAGCJHBWCKLJBsjkdohb";

    #[test]
    fn test_input_protocol_read_request_header_missing_op_id() {
        let input_prot_factory =
            FInputProtocolFactory::new(Box::new(TBinaryInputProtocolFactory::new()));
        let input_transport = Box::new(Cursor::new(BASIC_FRAME));
        let mut f_input_protocol = input_prot_factory.get_protocol(input_transport);
        let result = f_input_protocol.read_request_header();
        match result {
            Err(thrift::Error::Protocol(pe)) => {
                assert_eq!(thrift::ProtocolErrorKind::InvalidData, pe.kind);
                assert_eq!("frugal: request missing op id", &pe.message);
            }
            _ => panic!(
                "returned not an error, or wrong kind of error: {:?}",
                &result
            ),
        }
    }

    #[test]
    fn test_input_protocol_read_request_header_happy_path() {
        let input_prot_factory =
            FInputProtocolFactory::new(Box::new(TBinaryInputProtocolFactory::new()));
        let input_transport = Box::new(Cursor::new(FRUGAL_FRAME));
        let mut f_input_protocol = input_prot_factory.get_protocol(input_transport);
        let ctx = f_input_protocol.read_request_header().unwrap();
        assert_eq!(FRUGAL_CID, ctx.request_header(CID_HEADER).unwrap());
        assert_eq!("world", ctx.request_header("hello").unwrap());
    }

    #[test]
    fn test_input_protocol_read_response_header_happy_path() {
        let input_prot_factory =
            FInputProtocolFactory::new(Box::new(TBinaryInputProtocolFactory::new()));
        let input_transport = Box::new(Cursor::new(BASIC_FRAME));
        let mut f_input_protocol = input_prot_factory.get_protocol(input_transport);
        let mut ctx = FContextImpl::new(None);
        f_input_protocol.read_response_header(&mut ctx).unwrap();
        assert_eq!("bar", ctx.response_header("foo").unwrap());
    }

    #[test]
    fn test_output_protocol_write_request_header_errored_writer() {
        // TODO: find a better mocking story
        struct MockTransport;
        impl Write for MockTransport {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::TimedOut, "write failed"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let output_protocol_factory =
            FOutputProtocolFactory::new(Box::new(TBinaryOutputProtocolFactory::new()));
        let output_transport = Box::new(MockTransport);
        let mut f_output_protocol = output_protocol_factory.get_protocol(output_transport);
        let mut ctx = FContextImpl::new(None);
        ctx.add_request_header("foo", "bar");
        let result = f_output_protocol.write_request_header(&ctx);
        match result {
            Err(thrift::Error::Transport(te)) => {
                assert_eq!(thrift::TransportErrorKind::Unknown, te.kind);
                assert_eq!(
                    "frugal: error writing protocol headers in writeHeader: write failed",
                    &te.message
                );
            }
            _ => panic!(
                "returned not an error, or wrong kind of error: {:?}",
                &result
            ),
        }
    }

    #[test]
    fn test_output_protocol_write_request_header_bad_write() {
        // TODO: find a better mocking story
        struct MockTransport;
        impl Write for MockTransport {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Ok(0)
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let output_protocol_factory =
            FOutputProtocolFactory::new(Box::new(TBinaryOutputProtocolFactory::new()));
        let output_transport = Box::new(MockTransport);
        let mut f_output_protocol = output_protocol_factory.get_protocol(output_transport);
        let mut ctx = FContextImpl::new(None);
        ctx.add_request_header("foo", "bar");
        let result = f_output_protocol.write_request_header(&ctx);
        match result {
            Err(thrift::Error::Transport(te)) => {
                assert_eq!(thrift::TransportErrorKind::Unknown, te.kind);
                assert_eq!(
                    "frugal: failed to write complete protocol headers",
                    &te.message
                );
            }
            _ => panic!(
                "returned not an error, or wrong kind of error: {:?}",
                &result
            ),
        }
    }

    #[test]
    fn test_output_protocol_write_request_header_happy_path() {
        // TODO: find a better mocking story
        struct MockTransport;
        impl Write for MockTransport {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                assert_eq!(buf[0], 0); // protocol version
                assert_eq!(&buf[1..5], &[0u8, 0, 0, (buf.len() - 5) as u8]); // frame size
                assert_eq!(&buf[5..9], &[0u8, 0, 0, 4]); // _cid key size
                assert_eq!(&buf[9..13], "_cid".as_bytes()); // _cid key
                assert_eq!(&buf[13..17], &[0u8, 0, 0, 32]); // _cid value size
                                                            // skipping _cid value as it's random
                assert_eq!(&buf[49..53], &[0u8, 0, 0, 5]); // _opid key size
                assert_eq!(&buf[53..58], "_opid".as_bytes()); // _opid key
                                                              //assert_eq!(&buf[58..62], &[0u8, 0, 0, 1]); // _opid value size
                let s = buf[61] as usize; // need to know the _opid size for buf slices below
                                          // skipping _opid value as it could change
                assert_eq!(&buf[(62 + s)..(66 + s)], &[0u8, 0, 0, 8]); // _timeout key size
                assert_eq!(&buf[(66 + s)..(74 + s)], "_timeout".as_bytes()); // _timeout key
                assert_eq!(&buf[(74 + s)..(78 + s)], &[0u8, 0, 0, 4]); // _timeout value size
                assert_eq!(&buf[(78 + s)..(82 + s)], &[53u8, 48, 48, 48]); // _timeout value
                assert_eq!(&buf[(82 + s)..(86 + s)], &[0u8, 0, 0, 3]); // foo key size
                assert_eq!(&buf[(86 + s)..(89 + s)], "foo".as_bytes()); // foo key
                assert_eq!(&buf[(89 + s)..(93 + s)], &[0u8, 0, 0, 3]); // foo value size
                assert_eq!(&buf[(93 + s)..(96 + s)], "bar".as_bytes()); // foo value (bar)
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let output_protocol_factory =
            FOutputProtocolFactory::new(Box::new(TBinaryOutputProtocolFactory::new()));
        let output_transport = Box::new(MockTransport);
        let mut f_output_protocol = output_protocol_factory.get_protocol(output_transport);
        let mut ctx = FContextImpl::new(None);
        ctx.add_request_header("foo", "bar");
        f_output_protocol.write_request_header(&ctx).unwrap();
    }

    static WRITE_READ_REQUEST_HEADER_SYMMETRIC_FILE_PATH: &'static str =
        "write_read_request_header_symmetric_file_path.txt";

    #[test]
    fn test_write_read_request_header_symmetric() {
        // create ctx
        let mut ctx = FContextImpl::new(Some("123"));
        ctx.add_request_header("foo", "bar");
        ctx.add_request_header("hello", "world");
        let op_id = ctx.request_header(OP_ID_HEADER).unwrap();

        // call write_request_header
        {
            // NOTE: writing to file so I can easily read it back, no good way to do that in memory
            // due to the nature of the trait objects used in the thrift api
            let test_file =
                fs::File::create(WRITE_READ_REQUEST_HEADER_SYMMETRIC_FILE_PATH).unwrap();
            let output_protocol_factory =
                FOutputProtocolFactory::new(Box::new(TBinaryOutputProtocolFactory::new()));
            let mut f_output_protocol = output_protocol_factory.get_protocol(Box::new(test_file));
            f_output_protocol.write_request_header(&ctx).unwrap();
        }

        // read it with read_request_header
        let result = {
            let test_file = fs::File::open(WRITE_READ_REQUEST_HEADER_SYMMETRIC_FILE_PATH).unwrap();
            let input_prot_factory =
                FInputProtocolFactory::new(Box::new(TBinaryInputProtocolFactory::new()));
            let mut f_input_protocol = input_prot_factory.get_protocol(Box::new(test_file));
            f_input_protocol.read_request_header().unwrap()
        };

        // assert that the deserialized context is the same
        assert_eq!("world", result.request_header("hello").unwrap());
        assert_eq!("bar", result.request_header("foo").unwrap());
        assert_eq!("123", result.correlation_id());
        assert!(op_id != result.request_header(OP_ID_HEADER).unwrap());
        assert_eq!(op_id, result.response_header(OP_ID_HEADER).unwrap());

        // clean up test file
        fs::remove_file(WRITE_READ_REQUEST_HEADER_SYMMETRIC_FILE_PATH).unwrap();
    }

    static WRITE_READ_RESPONSE_HEADER_SYMMETRIC_FILE_PATH: &'static str =
        "write_read_response_header_symmetric_file_path.txt";

    #[test]
    fn test_write_read_response_header_symmetric() {
        // create ctx
        let mut ctx = FContextImpl::new(Some("123"));
        ctx.add_response_header("foo", "bar");
        ctx.add_response_header("hello", "world");
        let op_id = ctx.request_header(OP_ID_HEADER).unwrap().clone();
        ctx.add_response_header(OP_ID_HEADER, &op_id);

        // call write_response_header
        {
            // NOTE: writing to file so I can easily read it back, no good way to do that in memory
            // due to the nature of the trait objects used in the thrift api
            let test_file =
                fs::File::create(WRITE_READ_RESPONSE_HEADER_SYMMETRIC_FILE_PATH).unwrap();
            let output_protocol_factory =
                FOutputProtocolFactory::new(Box::new(TBinaryOutputProtocolFactory::new()));
            let mut f_output_protocol = output_protocol_factory.get_protocol(Box::new(test_file));
            f_output_protocol.write_response_header(&ctx).unwrap();
        }

        // read it with read_request_header
        let result = {
            let mut result_ctx = FContextImpl::new(Some("123"));
            let test_file = fs::File::open(WRITE_READ_RESPONSE_HEADER_SYMMETRIC_FILE_PATH).unwrap();
            let input_prot_factory =
                FInputProtocolFactory::new(Box::new(TBinaryInputProtocolFactory::new()));
            let mut f_input_protocol = input_prot_factory.get_protocol(Box::new(test_file));
            f_input_protocol
                .read_response_header(&mut result_ctx)
                .unwrap();
            result_ctx
        };

        // assert that the deserialized context is the same
        assert_eq!("world", result.response_header("hello").unwrap());
        assert_eq!("bar", result.response_header("foo").unwrap());
        assert_eq!("123", result.correlation_id());
        assert!(result.response_header(OP_ID_HEADER).is_none());

        // clean up test file
        fs::remove_file(WRITE_READ_RESPONSE_HEADER_SYMMETRIC_FILE_PATH).unwrap();
    }
}
