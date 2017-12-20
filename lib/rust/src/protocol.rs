use thrift::protocol::{TInputProtocol, TInputProtocolFactory, TOutputProtocol,
                       TOutputProtocolFactory};

pub trait FInputProtocol: TInputProtocol {}
pub trait FInputProtocolFactory: TInputProtocolFactory {}
pub trait FOutputProtocol: TOutputProtocol {}
pub trait FOutputProtocolFactory: TOutputProtocolFactory {}
