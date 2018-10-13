use thrift;
use thrift::protocol::TOutputProtocol;

use context::FContext;
use protocol::FOutputProtocol;

pub const APPLICATION_EXCEPTION_RESPONSE_TOO_LARGE: &'static str = "frugal: response was too large";
pub const APPLICATION_EXCEPTION_REQUEST_TOO_LARGE: &'static str = "frugal: request was too large";

pub fn write_application_error<S>(
    method: S,
    ctx: &mut FContext,
    err: &thrift::ApplicationError,
    oprot: &mut FOutputProtocol,
) -> thrift::Result<()>
where
    S: Into<String>,
{
    oprot.write_response_header(ctx)?;
    oprot.write_message_begin(&thrift::protocol::TMessageIdentifier {
        name: method.into(),
        message_type: thrift::protocol::TMessageType::Exception,
        sequence_number: 0,
    })?;
    thrift::Error::write_application_error_to_out_protocol(err, oprot)?;
    oprot.write_message_end()?;
    oprot.flush()
}
