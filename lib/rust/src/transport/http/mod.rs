use http::header;

pub static PAYLOAD_LIMIT: &str = "x-frugal-payload-limit";
pub static CONTENT_TRANSFER_ENCODING: &str = "content-transfer-encoding";
pub static FRUGAL_CONTENT_TYPE: &str = "application/x-frugal";
pub static BASE64_ENCODING: &str = "base64";

// In http <2.0, these are constants that have interior mutability so using them directly was
// triggering a severe clippy lint. So instead of using them directly I set them as statics here
// and import those rather than using them directly.
pub static CONTENT_LENGTH: header::HeaderName = header::CONTENT_LENGTH;
pub static CONTENT_TYPE: header::HeaderName = header::CONTENT_TYPE;

pub mod client;

pub mod server;
