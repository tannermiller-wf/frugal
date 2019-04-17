use std::error::Error;
use std::io;

use byteorder::{BigEndian, ReadBytesExt};
use thrift;

pub fn read_exact<R: io::Read>(reader: &mut R, size: usize) -> thrift::Result<Vec<u8>> {
    let mut buf = vec![0; size];
    reader.read_exact(&mut buf).map_err(|err| {
        match err.kind() {
                        io::ErrorKind::UnexpectedEof => thrift::new_transport_error(thrift::TransportErrorKind::EndOfFile, err.description()),
                        _ => thrift::new_transport_error(thrift::TransportErrorKind::Unknown, format!("frugal: error reading protocol headers in unmarshalHeaders reading header size: {}", err.description())),
                    }
    })?;
    Ok(buf)
}

pub fn read_size<R: io::Read>(reader: &mut R) -> thrift::Result<usize> {
    let buf = read_exact(reader, 4)?;
    let mut buf_reader = io::Cursor::new(buf);
    buf_reader
        .read_u32::<BigEndian>()
        .map_err(std::convert::Into::into)
        .map(|size| size as usize)
}
