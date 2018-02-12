use thrift::transport::TReadTransport;
use thrift;

use context::FContextImpl;

pub mod http;

pub trait FTransport {
    fn oneway(&self, ctx: &FContextImpl, payload: &[u8]) -> Option<thrift::Result<()>>;
    fn request(
        &self,
        ctx: &FContextImpl,
        payload: &[u8],
    ) -> Option<thrift::Result<Box<TReadTransport>>>;
    fn get_request_size_limit(&self) -> Option<usize>;
}
