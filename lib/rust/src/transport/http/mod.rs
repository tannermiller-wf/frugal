use hyper::header::HeaderName;
use mime;

lazy_static! {
    pub static ref PAYLOAD_LIMIT: HeaderName = HeaderName::from_static("x-frugal-payload-limit");
    pub static ref CONTENT_TRANSFER_ENCODING: HeaderName =
        HeaderName::from_static("content-transfer-encoding");
    pub static ref FRUGAL_CONTENT_TYPE: mime::Mime = "application/x-frugal".parse().unwrap();
}

pub static BASE64_ENCODING: &'static str = "base64";

mod client;
//pub use self::client::{FHttpTransport, FHttpTransportBuilder};

mod server;
//pub use self::server::FHttpService;
