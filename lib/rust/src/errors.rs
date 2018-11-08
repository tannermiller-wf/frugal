use thrift;
use thrift::protocol::TOutputProtocol;

use context::FContext;
use protocol::FOutputProtocol;

pub const USER_ERROR_DESCRIPTION: &'static str =
    "frugal: user handler code returned unhandled error on request with correlation id";

pub const APPLICATION_EXCEPTION_RESPONSE_TOO_LARGE: &'static str = "frugal: response was too large";
pub const APPLICATION_EXCEPTION_REQUEST_TOO_LARGE: &'static str = "frugal: request was too large";

pub fn write_application_error<S>(
    method: S,
    ctx: &FContext,
    err: &thrift::ApplicationError,
    oprot: &mut FOutputProtocol,
) -> thrift::Result<()>
where
    S: Into<String>,
{
    oprot.write_response_header(ctx)?;
    oprot.write_message_begin(&thrift::protocol::TMessageIdentifier::new(
        method,
        thrift::protocol::TMessageType::Exception,
        0,
    ))?;
    thrift::Error::write_application_error_to_out_protocol(err, oprot)?;
    oprot.write_message_end()?;
    oprot.flush()
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
