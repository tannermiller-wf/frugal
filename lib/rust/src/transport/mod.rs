use thrift;
use thrift::transport::TReadTransport;

use crate::context::FContext;

pub mod http;
pub mod nats;

pub trait FTransport: Clone {
    type Response: TReadTransport;

    fn oneway(&mut self, ctx: &FContext, payload: &[u8]) -> thrift::Result<()>;
    fn request(&mut self, ctx: &FContext, payload: &[u8]) -> thrift::Result<Self::Response>;
    fn get_request_size_limit(&self) -> Option<usize>;
}
