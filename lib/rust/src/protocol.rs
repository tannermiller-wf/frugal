use std::collections::BTreeMap;
use std::error::Error;
use std::io;
use std::sync::{Arc, Mutex};

use byteorder::{BigEndian, ReadBytesExt};
use thrift;
use thrift::protocol::{TInputProtocol, TInputProtocolFactory, TOutputProtocol,
                       TOutputProtocolFactory};
use thrift::transport::{TReadTransport, TWriteTransport};

use context::{self, FContext, FContextImpl};

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

fn read_exact<R: io::Read>(reader: &mut R, size: usize) -> thrift::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(size);
    buf.resize(size, 0);
    reader.read_exact(&mut buf).map_err(|err| {
        match err.kind() {
                        io::ErrorKind::UnexpectedEof => thrift::new_transport_error(thrift::TransportErrorKind::EndOfFile, err.description()),
                        _ => thrift::new_transport_error(thrift::TransportErrorKind::Unknown, format!("frugal: error reading protocol headers in unmarshalHeaders reading header size: {}", err.description())),
                    }
    })?;
    Ok(buf)
}

fn read_size<R: io::Read>(reader: &mut R) -> thrift::Result<usize> {
    let buf = read_exact(reader, 4)?;
    let mut buf_reader = io::Cursor::new(buf);
    buf_reader
        .read_u32::<BigEndian>()
        .map_err(|err| err.into())
        .map(|size| size as usize)
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
}

pub struct FInputProtocol {
    transport: Box<TReadTransport + Send>,
    protocol: Box<TInputProtocol + Send>,
}

// TODO: Should these deal with the FContext trait and not the impl?
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
    pub fn write_request_header(ctx: &FContextImpl) -> thrift::Result<()> {
        unimplemented!()
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
    use std::io::Cursor;

    use thrift;
    use thrift::protocol::TBinaryInputProtocolFactory;

    use super::*;
    use context::CID_HEADER;

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
        assert_eq!("bar", ctx.response_headers().get("foo").unwrap());
    }
}
