use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::time::Duration;

use uuid::Uuid;

// Header containing correlation id
pub static CID_HEADER: &'static str = "_cid";

// Header containing op id (uint64 as string)
pub static OP_ID_HEADER: &'static str = "_opid";

// Header containing request timeout (milliseconds as string)
pub static TIMEOUT_HEADER: &'static str = "_timeout";

lazy_static! {
    // Default request timeout
    static ref DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
}

fn duration_to_ms_string(d: &Duration) -> String {
    format!("{}", d.as_secs() * 1000 + d.subsec_nanos() as u64 / 1000000)
}

// used to track the op ids
static NEXT_OP_ID: AtomicUsize = ATOMIC_USIZE_INIT;

fn get_next_op_id() -> String {
    format!("{}", NEXT_OP_ID.fetch_add(1, Ordering::SeqCst))
}

// TODO: Document the tradeoffs in implementing this in Rust vs Go, e.g. Don't need an internal
//       mutex as that is handled externally.
#[derive(Debug)]
pub struct FContext {
    request_headers: BTreeMap<String, String>,
    response_headers: BTreeMap<String, String>,
}

impl FContext {
    pub fn new(correlation_id: Option<&str>) -> Self {
        let cid = match correlation_id {
            Some(cid) => cid.to_string(),
            None => Uuid::new_v4().to_simple().to_string(),
        };

        let mut request = BTreeMap::new();
        request.insert(CID_HEADER.to_string(), cid.to_string());
        request.insert(
            TIMEOUT_HEADER.to_string(),
            duration_to_ms_string(&DEFAULT_TIMEOUT),
        );
        request.insert(OP_ID_HEADER.to_string(), get_next_op_id());

        FContext {
            request_headers: request,
            response_headers: BTreeMap::new(),
        }
    }

    pub fn correlation_id(&self) -> &str {
        // correlation id should always exist
        &self.request_headers[CID_HEADER]
    }

    pub fn add_request_header<S: Into<String>>(&mut self, name: S, value: S) -> &mut Self {
        self.request_headers.insert(name.into(), value.into());
        self
    }

    pub fn request_header(&self, name: &str) -> Option<&String> {
        self.request_headers.get(name)
    }

    pub fn request_headers(&self) -> &BTreeMap<String, String> {
        &self.request_headers
    }

    pub fn add_response_header<S: Into<String>>(&mut self, name: S, value: S) -> &mut Self {
        self.response_headers.insert(name.into(), value.into());
        self
    }

    pub fn response_header(&self, name: &str) -> Option<&String> {
        self.response_headers.get(name)
    }

    pub fn response_headers(&self) -> &BTreeMap<String, String> {
        &self.response_headers
    }

    pub fn set_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.request_headers
            .insert(TIMEOUT_HEADER.to_string(), duration_to_ms_string(&timeout));
        self
    }

    pub fn timeout(&self) -> Duration {
        match self.request_headers.get(TIMEOUT_HEADER) {
            Some(timeout) => timeout
                .parse()
                .map(Duration::from_millis)
                .unwrap_or_else(|_| DEFAULT_TIMEOUT.clone()),
            None => DEFAULT_TIMEOUT.clone(),
        }
    }

    pub fn get_op_id(&self) -> Result<u64, String> {
        self.request_headers
            .get(OP_ID_HEADER)
            .ok_or(format!(
                "FContext does not have the required {} request header",
                OP_ID_HEADER
            )).and_then(|op_id| {
                op_id
                    .parse()
                    .map_err(|err: std::num::ParseIntError| err.to_string())
            })
    }
}

impl Clone for FContext {
    fn clone(&self) -> Self {
        FContext {
            request_headers: {
                let mut rh = self.request_headers.clone();
                rh.insert(OP_ID_HEADER.to_string(), get_next_op_id());
                rh
            },
            response_headers: self.response_headers.clone(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_corr_id() {
        let ctx = FContext::new(Some("fooid"));
        assert_eq!("fooid", ctx.correlation_id());

        let ctx2 = FContext::new(None);
        assert!(ctx2.correlation_id() != "");
    }

    #[test]
    fn test_new_corr_id() {
        let ctx = FContext::new(None);
        assert!(ctx.correlation_id() != "");
    }

    #[test]
    fn test_timeout() {
        let mut ctx = FContext::new(None);
        assert_eq!(Duration::from_secs(5), ctx.timeout());

        ctx.set_timeout(Duration::from_secs(10));
        assert_eq!(Duration::from_secs(10), ctx.timeout());
    }

    #[test]
    fn test_request_header() {
        let cid = "cid";
        let mut ctx = FContext::new(Some(cid));
        ctx.add_request_header("foo", "bar")
            .add_request_header("baz", "qux");
        assert_eq!(Some(&"bar".to_string()), ctx.request_header("foo"));
        assert_eq!(Some(&"qux".to_string()), ctx.request_header("baz"));
        assert_eq!(Some(&"bar".to_string()), ctx.request_headers().get("foo"));
        assert_eq!(Some(&cid.to_string()), ctx.request_header(CID_HEADER));
    }

    #[test]
    fn test_response_header() {
        let cid = "cid";
        let mut ctx = FContext::new(Some(cid));
        ctx.add_response_header("foo", "bar")
            .add_response_header("baz", "qux");
        assert_eq!(Some(&"bar".to_string()), ctx.response_header("foo"));
        assert_eq!(Some(&"qux".to_string()), ctx.response_header("baz"));
    }

    #[test]
    fn test_clone() {
        let mut ctx = FContext::new(Some("some-id"));
        ctx.add_request_header("foo", "bar");
        let cloned = ctx.clone();
        assert!(
            ctx.request_headers().get(OP_ID_HEADER) != cloned.request_headers().get(OP_ID_HEADER)
        );
        assert_eq!(
            ctx.request_headers().get(TIMEOUT_HEADER),
            cloned.request_headers().get(TIMEOUT_HEADER)
        );
        assert_eq!(
            ctx.request_headers().get(CID_HEADER),
            cloned.request_headers().get(CID_HEADER)
        );
        assert_eq!(
            ctx.request_headers().get("foo"),
            cloned.request_headers().get("foo")
        );
    }
}
