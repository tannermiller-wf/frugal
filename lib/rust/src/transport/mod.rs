use thrift::transport::TReadTransport;
use thrift;

use context::FContext;

pub mod http;

pub trait FTransport {
    fn oneway(&mut self, ctx: &FContext, payload: &[u8]) -> Option<thrift::Result<()>>;
    fn request(
        &mut self,
        ctx: &FContext,
        payload: &[u8],
    ) -> Option<thrift::Result<Box<TReadTransport>>>;
    fn get_request_size_limit(&self) -> Option<usize>;
}
