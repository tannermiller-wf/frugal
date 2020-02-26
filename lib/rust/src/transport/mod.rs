use async_trait::async_trait;
use thrift;
use thrift::transport::TReadTransport;

use crate::context::FContext;

pub mod http;
pub mod nats;

#[async_trait]
pub trait FTransport: Clone {
    type Response: TReadTransport;

    async fn oneway(&self, ctx: &FContext, payload: &[u8]) -> thrift::Result<()>;
    async fn request(&self, ctx: &FContext, payload: &[u8]) -> thrift::Result<Self::Response>;
    fn get_request_size_limit(&self) -> Option<usize>;
}
