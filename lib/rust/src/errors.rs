use thrift;
use thrift::protocol::TOutputProtocol;
use thrift::transport::TWriteTransport;

use context::FContext;
use protocol::FOutputProtocol;

pub const USER_ERROR_DESCRIPTION: &str =
    "frugal: user handler code returned unhandled error on request with correlation id";

pub const APPLICATION_EXCEPTION_RESPONSE_TOO_LARGE: &str = "frugal: response was too large";
pub const APPLICATION_EXCEPTION_REQUEST_TOO_LARGE: &str = "frugal: request was too large";

pub fn write_application_error<S, W>(
    method: S,
    ctx: &FContext,
    err: &thrift::ApplicationError,
    oprot: &mut FOutputProtocol<W>,
) -> thrift::Result<()>
where
    S: Into<String>,
    W: TWriteTransport,
{
    oprot.write_response_header(ctx)?;
    let mut proxy = oprot.t_protocol_proxy();
    proxy.write_message_begin(&thrift::protocol::TMessageIdentifier::new(
        method,
        thrift::protocol::TMessageType::Exception,
        0,
    ))?;
    thrift::Error::write_application_error_to_out_protocol(err, &mut proxy)?;
    proxy.write_message_end()?;
    proxy.flush()
}

pub fn is_too_large_error(err: &thrift::Error) -> bool {
    // TODO: I'm not sure if these will always be in TransportErrors, maybe just do the ckeck for
    // everything
    if let thrift::Error::Transport(ref err) = err {
        let err_string = err.to_string();
        err_string.contains(APPLICATION_EXCEPTION_RESPONSE_TOO_LARGE)
            || err_string.contains(APPLICATION_EXCEPTION_REQUEST_TOO_LARGE)
    } else {
        false
    }
}
