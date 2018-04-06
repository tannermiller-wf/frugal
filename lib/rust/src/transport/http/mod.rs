use mime;

header!{ (PayloadLimit, "x-frugal-payload-limit") => [i64] }
header!{ (ContentTransferEncodingHeader, "content-transfer-encoding") => [String] }

pub static BASE64_ENCODING: &'static str = "base64";

lazy_static! {
    pub static ref FRUGAL_CONTENT_TYPE: mime::Mime = "application/x-frugal".parse().unwrap();
}

mod client;
pub use self::client::{FHttpTransport, FHttpTransportBuilder};

mod server;
pub use self::server::FHttpService;
