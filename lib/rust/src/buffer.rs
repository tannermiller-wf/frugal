use std::io::{Result as IoResult, Write};

const EMPTY_FRAME_SIZE: &'static [u8] = &[0, 0, 0, 0];

pub struct FMemoryOutputBuffer {
    limit: usize,
    buffer: Vec<u8>,
}

impl FMemoryOutputBuffer {
    pub fn new(limit: usize) -> FMemoryOutputBuffer {
        let mut buffer = Vec::with_capacity(limit);
        buffer
            .write(EMPTY_FRAME_SIZE)
            .expect("This should never happen: FMemoryOutputBuffer::new()");
        FMemoryOutputBuffer { limit, buffer }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.buffer
    }

    pub fn reset(&mut self) {
        self.buffer.truncate(0);
        self.buffer
            .write(EMPTY_FRAME_SIZE)
            .expect("This should never happen: FMemoryOutputBuffer::reset()");
    }
}

impl Write for FMemoryOutputBuffer {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        if self.limit > 0 && self.buffer.len() + buf.len() > self.limit {
            self.reset();
            // TODO: Fix this
            //return Err(thrift::new_transport_error(
            //    thrift::TransportErrorKind::SizeLimit,
            //    format!("Buffer size reached ({})", self.limit),
            //));
        }
        self.buffer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.buffer.flush()
    }
}
