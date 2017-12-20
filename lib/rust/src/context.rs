use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

use chrono::Duration;
use uuid::Uuid;

// Header containing correlation id
static CID_HEADER: &'static str = "_cid";

// Header containing op id (uint64 as string)
static OP_ID_HEADER: &'static str = "_opid";

// Header containing request timeout (milliseconds as string)
static TIMEOUT_HEADER: &'static str = "_timeout";

lazy_static! {
    // Default request timeout
    static ref DEFAULT_TIMEOUT: Duration = Duration::seconds(5);
}

// used to track the op ids
static NEXT_OP_ID: AtomicUsize = ATOMIC_USIZE_INIT;

fn get_next_op_id() -> String {
    format!("{}", NEXT_OP_ID.fetch_add(1, Ordering::SeqCst))
}

pub trait FContext: Clone {
    // correlation_id returns the correlation id for the context.
    fn correlation_id(&self) -> &str;

    // add_request_header adds a request header to the context for the given
    // name. The headers _cid and _opid are reserved. Returns the same FContext
    // to allow for chaining calls.
    fn add_request_header<S: Into<String>>(&mut self, name: S, value: S) -> &mut Self;

    // request_header gets the named request header.
    fn request_header(&self, name: &str) -> Option<&String>;

    // request_headers returns the request headers map.
    fn request_headers(&self) -> &BTreeMap<String, String>;

    // add_response_header adds a response header to the context for the given
    // name. The _opid header is reserved. Returns the same FContext to allow
    // for chaining calls.
    fn add_response_header<S: Into<String>>(&mut self, name: S, value: S) -> &mut Self;

    // response_headers returns the response headers map.
    fn response_headers(&self) -> &BTreeMap<String, String>;

    // set_timeout sets the request timeout. Default is 5 seconds. Returns the
    // same FContext to allow for chaining calls.
    fn set_timeout(&mut self, timeout: Duration) -> &mut Self;

    fn timeout(&self) -> Duration;
}

pub struct FContextImpl {
    request_headers: BTreeMap<String, String>,
    response_headers: BTreeMap<String, String>,
}

impl FContextImpl {
    pub fn new(correlation_id: Option<&str>) -> Self {
        let cid = match correlation_id {
            Some(cid) => cid.to_string(),
            None => Uuid::new_v4().simple().to_string(),
        };

        let mut request = BTreeMap::new();
        request.insert(CID_HEADER.to_string(), cid.to_string());
        request.insert(
            TIMEOUT_HEADER.to_string(),
            format!("{}", DEFAULT_TIMEOUT.num_milliseconds()),
        );
        request.insert(OP_ID_HEADER.to_string(), get_next_op_id());

        FContextImpl {
            request_headers: request,
            response_headers: BTreeMap::new(),
        }
    }
}

impl Clone for FContextImpl {
    fn clone(&self) -> Self {
        FContextImpl {
            request_headers: {
                let mut rh = self.request_headers.clone();
                rh.insert(OP_ID_HEADER.to_string(), get_next_op_id());
                rh
            },
            response_headers: self.response_headers.clone(),
        }
    }
}

impl FContext for FContextImpl {
    fn correlation_id(&self) -> &str {
        // correlation id should always exist
        &self.request_headers[CID_HEADER]
    }

    fn add_request_header<S: Into<String>>(&mut self, name: S, value: S) -> &mut Self {
        self.request_headers.insert(name.into(), value.into());
        self
    }

    fn request_header(&self, name: &str) -> Option<&String> {
        self.request_headers.get(name)
    }

    fn request_headers(&self) -> &BTreeMap<String, String> {
        &self.request_headers
    }

    fn add_response_header<S: Into<String>>(&mut self, name: S, value: S) -> &mut Self {
        self.response_headers.insert(name.into(), value.into());
        self
    }

    fn response_headers(&self) -> &BTreeMap<String, String> {
        &self.response_headers
    }

    fn set_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.request_headers.insert(
            TIMEOUT_HEADER.to_string(),
            format!("{}", timeout.num_milliseconds()),
        );
        self
    }

    fn timeout(&self) -> Duration {
        match self.request_headers.get(TIMEOUT_HEADER) {
            Some(timeout) => timeout
                .parse()
                .map(Duration::milliseconds)
                .unwrap_or_else(|_| DEFAULT_TIMEOUT.clone()),
            None => DEFAULT_TIMEOUT.clone(),
        }
    }
}

mod test {
    use super::*;

    #[test]
    fn test_corr_id() {
        let ctx = FContextImpl::new(Some("fooid"));
        assert_eq!("fooid", ctx.correlation_id());

        let ctx2 = FContextImpl::new(None);
        assert!(ctx2.correlation_id() != "");
    }

    #[test]
    fn test_new_corr_id() {
        let ctx = FContextImpl::new(None);
        assert!(ctx.correlation_id() != "");
    }

    #[test]
    fn test_timeout() {
        let mut ctx = FContextImpl::new(None);
        assert_eq!(Duration::seconds(5), ctx.timeout());

        ctx.set_timeout(Duration::seconds(10));
        assert_eq!(Duration::seconds(10), ctx.timeout());
    }

    #[test]
    fn test_request_header() {
        let cid = "cid";
        let mut ctx = FContextImpl::new(Some(cid));
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
        let mut ctx = FContextImpl::new(Some(cid));
        ctx.add_response_header("foo", "bar")
            .add_response_header("baz", "qux");
        assert_eq!(Some(&"bar".to_string()), ctx.response_headers().get("foo"));
        assert_eq!(Some(&"qux".to_string()), ctx.response_headers().get("baz"));
    }

    #[test]
    fn test_clone() {
        let mut ctx = FContextImpl::new(Some("some-id"));
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
