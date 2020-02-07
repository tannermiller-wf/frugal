pub static PAYLOAD_LIMIT: &'static str = "x-frugal-payload-limit";
pub static CONTENT_TRANSFER_ENCODING: &'static str = "content-transfer-encoding";
pub static FRUGAL_CONTENT_TYPE: &'static str = "application/x-frugal";

pub static BASE64_ENCODING: &'static str = "base64";

mod client;
//pub use self::client::{FHttpTransport, FHttpTransportBuilder};

pub mod server;
//pub use self::server::FHttpService;
