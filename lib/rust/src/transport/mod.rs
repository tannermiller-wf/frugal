use thrift;
use thrift::transport::TReadTransport;

use context::FContext;

pub mod http;

pub trait FTransport {
    fn oneway(&mut self, ctx: &FContext, payload: &[u8]) -> Option<thrift::Result<()>>;
    // TODO: See if we can make this parametric
    fn request(&mut self, ctx: &FContext, payload: &[u8]) -> thrift::Result<Box<TReadTransport>>;
    fn get_request_size_limit(&self) -> Option<usize>;
}
