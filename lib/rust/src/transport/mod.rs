use thrift;
use thrift::transport::TReadTransport;

use context::FContext;

pub mod http;
pub mod nats;

pub trait FTransport {
    type Response: TReadTransport;

    fn oneway(&mut self, ctx: &FContext, payload: &[u8]) -> thrift::Result<()>;
    fn request(&mut self, ctx: &FContext, payload: &[u8]) -> thrift::Result<Self::Response>;
    fn get_request_size_limit(&self) -> Option<usize>;
}
