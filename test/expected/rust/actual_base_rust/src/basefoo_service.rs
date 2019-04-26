// Autogenerated by Frugal Compiler (3.3.1)
// DO NOT EDIT UNLESS YOU ARE SURE THAT YOU KNOW WHAT YOU ARE DOING

#![allow(unused_variables)]

use std::collections::BTreeMap;
use std::error::Error;

use futures::future::{self, FutureResult};
use futures::{Async, Future, Poll};
use thrift;
use thrift::protocol::{TInputProtocol, TOutputProtocol};
use tower_service::Service;
use tower_web::middleware::{self, Middleware};
use tower_web::util::Chain;

use frugal::buffer::FMemoryOutputBuffer;
use frugal::context::{FContext, OP_ID_HEADER};
use frugal::errors;
use frugal::processor::FProcessor;
use frugal::protocol::{
    FInputProtocol, FInputProtocolFactory, FOutputProtocol, FOutputProtocolFactory,
};
use frugal::provider::FServiceProvider;
use frugal::transport::FTransport;

use super::*;

pub trait FBaseFoo {
    fn base_ping(&mut self, ctx: &FContext) -> thrift::Result<()>;
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct FBaseFooBasePingArgs {}

impl FBaseFooBasePingArgs {
    pub fn read<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    where
        R: thrift::transport::TReadTransport,
        T: thrift::protocol::TInputProtocol<R>,
    {
        iprot.read_struct_begin()?;
        loop {
            let field_id = iprot.read_field_begin()?;
            if field_id.field_type == thrift::protocol::TType::Stop {
                break;
            };
            match field_id.id {
                _ => iprot.skip(field_id.field_type)?,
            };
            iprot.read_field_end()?;
        }
        iprot.read_struct_end()
    }

    pub fn write<W, T>(&self, oprot: &mut T) -> thrift::Result<()>
    where
        W: thrift::transport::TWriteTransport,
        T: thrift::protocol::TOutputProtocol<W>,
    {
        oprot.write_struct_begin(&thrift::protocol::TStructIdentifier::new("basePing_args"))?;
        oprot.write_field_stop()?;
        oprot.write_struct_end()
    }
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct FBaseFooBasePingResult {}

impl FBaseFooBasePingResult {
    pub fn read<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    where
        R: thrift::transport::TReadTransport,
        T: thrift::protocol::TInputProtocol<R>,
    {
        iprot.read_struct_begin()?;
        loop {
            let field_id = iprot.read_field_begin()?;
            if field_id.field_type == thrift::protocol::TType::Stop {
                break;
            };
            match field_id.id {
                _ => iprot.skip(field_id.field_type)?,
            };
            iprot.read_field_end()?;
        }
        iprot.read_struct_end()
    }

    pub fn write<W, T>(&self, oprot: &mut T) -> thrift::Result<()>
    where
        W: thrift::transport::TWriteTransport,
        T: thrift::protocol::TOutputProtocol<W>,
    {
        oprot.write_struct_begin(&thrift::protocol::TStructIdentifier::new("basePing_result"))?;
        oprot.write_field_stop()?;
        oprot.write_struct_end()
    }
}

pub enum FBaseFooMethod {
    BasePing(FBaseFooBasePingArgs),
}

impl FBaseFooMethod {
    fn name(&self) -> &'static str {
        match *self {
            FBaseFooMethod::BasePing(_) => "basePing",
        }
    }
}

pub struct FBaseFooRequest {
    ctx: FContext,
    method: FBaseFooMethod,
}

impl FBaseFooRequest {
    pub fn new(ctx: FContext, method: FBaseFooMethod) -> FBaseFooRequest {
        FBaseFooRequest { ctx, method }
    }
}

impl frugal::service::Request for FBaseFooRequest {
    fn context(&mut self) -> &mut FContext {
        &mut self.ctx
    }

    fn method_name(&self) -> &'static str {
        self.method.name()
    }
}

pub enum FBaseFooResponse {
    BasePing(FBaseFooBasePingResult),
}

pub struct FBaseFooClient<S>
where
    S: Service<Request = FBaseFooRequest, Response = FBaseFooResponse, Error = thrift::Error>,
{
    service: S,
}

pub struct FBaseFooClientBuilder<M> {
    middleware: M,
}

impl FBaseFooClientBuilder<middleware::Identity> {
    pub fn new() -> Self {
        FBaseFooClientBuilder {
            middleware: middleware::Identity::new(),
        }
    }
}

impl<M> FBaseFooClientBuilder<M> {
    pub fn middleware<U>(self, middleware: U) -> FBaseFooClientBuilder<<M as Chain<U>>::Output>
    where
        M: Chain<U>,
    {
        FBaseFooClientBuilder {
            middleware: self.middleware.chain(middleware),
        }
    }

    pub fn build<T>(self, provider: FServiceProvider<T>) -> FBaseFooClient<M::Service>
    where
        T: FTransport,
        M: Middleware<
            FBaseFooClientService<T>,
            Request = FBaseFooRequest,
            Response = FBaseFooResponse,
            Error = thrift::Error,
        >,
    {
        FBaseFooClient {
            service: self.middleware.wrap(FBaseFooClientService {
                transport: provider.transport,
                input_protocol_factory: provider.input_protocol_factory,
                output_protocol_factory: provider.output_protocol_factory,
            }),
        }
    }
}

impl<S> FBaseFoo for FBaseFooClient<S>
where
    S: Service<Request = FBaseFooRequest, Response = FBaseFooResponse, Error = thrift::Error>,
{
    fn base_ping(&mut self, ctx: &FContext) -> thrift::Result<()> {
        let args = FBaseFooBasePingArgs {};
        let request = FBaseFooRequest::new(ctx.clone(), FBaseFooMethod::BasePing(args));
        match self.service.call(request).wait()? {
            FBaseFooResponse::BasePing(result) => Ok(()),
        }
    }
}

pub struct FBaseFooClientService<T>
where
    T: FTransport,
{
    transport: T,
    input_protocol_factory: FInputProtocolFactory,
    output_protocol_factory: FOutputProtocolFactory,
}

impl<T> FBaseFooClientService<T>
where
    T: FTransport,
{
    fn call_delegate(&mut self, req: FBaseFooRequest) -> Result<FBaseFooResponse, thrift::Error> {
        enum ResultSignifier {
            BasePing,
        };
        let FBaseFooRequest { mut ctx, method } = req;
        let method_name = method.name();
        let mut buffer = FMemoryOutputBuffer::new(0);
        let signifier = {
            let mut oprot = self.output_protocol_factory.get_protocol(&mut buffer);
            oprot.write_request_header(&ctx)?;
            let mut oproxy = oprot.t_protocol_proxy();
            let signifier = match method {
                FBaseFooMethod::BasePing(args) => {
                    oproxy.write_message_begin(&thrift::protocol::TMessageIdentifier::new(
                        "basePing",
                        thrift::protocol::TMessageType::Call,
                        0,
                    ))?;
                    let args = FBaseFooBasePingArgs {};
                    args.write(&mut oproxy)?;
                    ResultSignifier::BasePing
                }
            };
            oproxy.write_message_end()?;
            oproxy.flush()?;
            signifier
        };
        let mut result_transport = self.transport.request(&ctx, buffer.bytes())?;
        {
            let mut iprot = self
                .input_protocol_factory
                .get_protocol(&mut result_transport);
            iprot.read_response_header(&mut ctx)?;
            let mut iproxy = iprot.t_protocol_proxy();
            let msg_id = iproxy.read_message_begin()?;
            if msg_id.name != method_name {
                return Err(thrift::new_application_error(
                    thrift::ApplicationErrorKind::WrongMethodName,
                    format!("{} failed: wrong method name", method_name),
                ));
            }
            match msg_id.message_type {
                thrift::protocol::TMessageType::Exception => {
                    let err = thrift::Error::Application(
                        thrift::Error::read_application_error_from_in_protocol(&mut iproxy)?,
                    );
                    iproxy.read_message_end()?;
                    if frugal::errors::is_too_large_error(&err) {
                        Err(thrift::new_transport_error(
                            thrift::TransportErrorKind::SizeLimit,
                            err.to_string(),
                        ))
                    } else {
                        Err(err)
                    }
                }
                thrift::protocol::TMessageType::Reply => match signifier {
                    ResultSignifier::BasePing => {
                        let mut result = FBaseFooBasePingResult::default();
                        result.read(&mut iproxy)?;
                        iproxy.read_message_end()?;
                        Ok(FBaseFooResponse::BasePing(result))
                    }
                },
                _ => Err(thrift::new_application_error(
                    thrift::ApplicationErrorKind::InvalidMessageType,
                    format!("{} failed: invalid message type", method_name),
                )),
            }
        }
    }
}

impl<T> Service for FBaseFooClientService<T>
where
    T: FTransport,
{
    type Request = FBaseFooRequest;
    type Response = FBaseFooResponse;
    type Error = thrift::Error;
    type Future = FutureResult<Self::Response, Self::Error>;

    fn poll_ready(&mut self) -> Poll<(), thrift::Error> {
        Ok(Async::Ready(()))
    }

    fn call(&mut self, req: Self::Request) -> Self::Future {
        self.call_delegate(req).into()
    }
}

#[derive(Clone)]
pub struct FBaseFooProcessor<S>
where
    S: Service<Request = FBaseFooRequest, Response = FBaseFooResponse, Error = thrift::Error>,
{
    service: S,
}

pub struct FBaseFooProcessorBuilder<F, M>
where
    F: FBaseFoo,
{
    handler: F,
    middleware: M,
}

impl<F> FBaseFooProcessorBuilder<F, middleware::Identity>
where
    F: FBaseFoo,
{
    pub fn new(handler: F) -> Self {
        FBaseFooProcessorBuilder {
            handler,
            middleware: middleware::Identity::new(),
        }
    }
}

impl<F, M> FBaseFooProcessorBuilder<F, M>
where
    F: FBaseFoo + Clone,
{
    pub fn middleware<U>(
        self,
        middleware: U,
    ) -> FBaseFooProcessorBuilder<F, <M as Chain<U>>::Output>
    where
        M: Chain<U>,
    {
        FBaseFooProcessorBuilder {
            handler: self.handler,
            middleware: self.middleware.chain(middleware),
        }
    }

    pub fn build(self) -> FBaseFooProcessor<M::Service>
    where
        M: Middleware<
            FBaseFooProcessorService<F>,
            Request = FBaseFooRequest,
            Response = FBaseFooResponse,
            Error = thrift::Error,
        >,
    {
        FBaseFooProcessor {
            service: self.middleware.wrap(FBaseFooProcessorService(self.handler)),
        }
    }
}

#[derive(Clone)]
pub struct FBaseFooProcessorService<F: FBaseFoo + Clone>(F);

impl<F> Service for FBaseFooProcessorService<F>
where
    F: FBaseFoo + Clone,
{
    type Request = FBaseFooRequest;
    type Response = FBaseFooResponse;
    type Error = thrift::Error;
    type Future = FutureResult<Self::Response, Self::Error>;

    fn poll_ready(&mut self) -> Poll<(), thrift::Error> {
        Ok(Async::Ready(()))
    }

    fn call(&mut self, req: FBaseFooRequest) -> FutureResult<FBaseFooResponse, thrift::Error> {
        let result = match req.method {
            FBaseFooMethod::BasePing(args) => self
                .0
                .base_ping(&req.ctx)
                .map(|res| FBaseFooResponse::BasePing(FBaseFooBasePingResult {})),
        };
        future::result(result)
    }
}

impl<S> FProcessor for FBaseFooProcessor<S>
where
    S: Service<Request = FBaseFooRequest, Response = FBaseFooResponse, Error = thrift::Error>
        + Clone
        + Send
        + 'static,
{
    fn process<R, W>(
        &mut self,
        iprot: &mut FInputProtocol<R>,
        oprot: &mut FOutputProtocol<W>,
    ) -> thrift::Result<()>
    where
        R: thrift::transport::TReadTransport,
        W: thrift::transport::TWriteTransport,
    {
        let ctx = iprot.read_request_header()?;
        let name = {
            let mut iproxy = iprot.t_protocol_proxy();
            iproxy.read_message_begin().map(|tmid| tmid.name)?
        };

        match &*name {
            "basePing" => self.base_ping(&ctx, iprot, oprot),
            _ => {
                error!(
                    "frugal: client invoked unknown function {} on request with correlation id {}",
                    &name,
                    ctx.correlation_id()
                );
                let mut iproxy = iprot.t_protocol_proxy();
                iproxy.skip(thrift::protocol::TType::Struct)?;
                iproxy.read_message_end()?;

                oprot.write_response_header(&ctx)?;
                let mut oproxy = oprot.t_protocol_proxy();
                oproxy.write_message_begin(&thrift::protocol::TMessageIdentifier::new(
                    &name as &str,
                    thrift::protocol::TMessageType::Exception,
                    0,
                ))?;
                let ex = thrift::ApplicationError::new(
                    thrift::ApplicationErrorKind::UnknownMethod,
                    format!("Unknown function {}", &name),
                );
                thrift::Error::write_application_error_to_out_protocol(&ex, &mut oproxy)?;
                oproxy.write_message_end()?;
                oproxy.flush()
            }
        }
    }
}

impl<S> FBaseFooProcessor<S>
where
    S: Service<Request = FBaseFooRequest, Response = FBaseFooResponse, Error = thrift::Error>,
{
    fn base_ping<R, W>(
        &mut self,
        ctx: &FContext,
        iprot: &mut FInputProtocol<R>,
        oprot: &mut FOutputProtocol<W>,
    ) -> thrift::Result<()>
    where
        R: thrift::transport::TReadTransport,
        W: thrift::transport::TWriteTransport,
    {
        let mut args = FBaseFooBasePingArgs::default();
        let mut iproxy = iprot.t_protocol_proxy();
        args.read(&mut iproxy)?;
        iproxy.read_message_end()?;
        let req = FBaseFooRequest {
            ctx: ctx.clone(),
            method: FBaseFooMethod::BasePing(args),
        };
        match self.service.call(req).wait() {
            Err(thrift::Error::User(err)) => {
                error!(
                    "{} {}: {}",
                    errors::USER_ERROR_DESCRIPTION,
                    ctx.correlation_id(),
                    err.description()
                );
                Ok(())
            }
            Err(err) => {
                error!(
                    "{} {}: {}",
                    errors::USER_ERROR_DESCRIPTION,
                    ctx.correlation_id(),
                    err.description()
                );
                Ok(())
            }
            Ok(FBaseFooResponse::BasePing(result)) => oprot
                .write_response_header(&ctx)
                .and_then(|()| {
                    oprot.t_protocol_proxy().write_message_begin(
                        &thrift::protocol::TMessageIdentifier::new(
                            "basePing",
                            thrift::protocol::TMessageType::Reply,
                            0,
                        ),
                    )
                })
                .and_then(|()| result.write(&mut oprot.t_protocol_proxy()))
                .and_then(|()| oprot.t_protocol_proxy().write_message_end())
                .and_then(|()| oprot.t_protocol_proxy().flush())
                .or_else(|err| {
                    if errors::is_too_large_error(&err) {
                        errors::write_application_error(
                            "basePing",
                            &ctx,
                            &thrift::ApplicationError::new(
                                thrift::ApplicationErrorKind::Unknown,
                                errors::APPLICATION_EXCEPTION_RESPONSE_TOO_LARGE,
                            ),
                            oprot,
                        )
                    } else {
                        Err(err)
                    }
                }),
        }
    }
}
