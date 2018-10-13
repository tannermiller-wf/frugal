use futures::future::FutureResult;
use thrift;
use tower_service::Service;

use context::FContext;

// TODO: Figure out middleware, seems hard without reflection, look at adapting frugal to tower-rs
// Put all things we might want to get in here. This is intended to be a bound on a
// tower_service::Service::Request
pub trait Request {
    fn context(&mut self) -> &mut FContext; // I'm not sure if tower lets you mutate the request. But I think it would almost half to
    fn method_name(&self) -> &'static str; // TODO: Is static here fine? Would it ever change?
}

pub struct Method<S, Req, Res>
where
    Req: Request,
    S: Service<
        Request = Req,
        Response = Res,
        Error = thrift::Error,
        Future = FutureResult<Res, thrift::Error>,
    >,
{
    service: S,
}

#[allow(dead_code)]
mod example {
    use super::Request;
    use futures::{Async, Poll};
    use tower_service::Service;

    pub struct InsertHeader<T> {
        inner: T,
        key: String,
        value: String,
    }

    impl<T> InsertHeader<T> {
        pub fn new<S>(inner: T, key: S, value: S) -> InsertHeader<T>
        where
            S: Into<String>,
        {
            InsertHeader {
                inner: inner,
                key: key.into(),
                value: value.into(),
            }
        }
    }

    impl<T> Service for InsertHeader<T>
    where
        T: Service,
        T::Request: Request,
    {
        type Request = T::Request;
        type Response = T::Response;
        type Error = T::Error;
        type Future = T::Future;

        fn poll_ready(&mut self) -> Poll<(), Self::Error> {
            Ok(Async::Ready(()))
        }

        fn call(&mut self, mut req: Self::Request) -> Self::Future {
            req.context().add_request_header(&*self.key, &*self.value);
            self.inner.call(req)
        }
    }
}
