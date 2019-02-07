use crate::context::FContext;

pub trait Request {
    fn context(&mut self) -> &mut FContext;
    fn method_name(&self) -> &'static str; // TODO: Is static here fine? Would it ever change?
}

pub mod example {
    use super::Request;
    use futures::Poll;
    use tower_service::Service;
    use tower_web::middleware::Middleware;

    pub struct InsertHeaderMiddleware {
        key: String,
        value: String,
    }

    impl InsertHeaderMiddleware {
        pub fn new<S>(key: S, value: S) -> InsertHeaderMiddleware
        where
            S: Into<String>,
        {
            InsertHeaderMiddleware {
                key: key.into(),
                value: value.into(),
            }
        }
    }

    impl<S> Middleware<S> for InsertHeaderMiddleware
    where
        S: Service,
        S::Request: Request,
    {
        type Request = S::Request;
        type Response = S::Response;
        type Error = S::Error;
        type Service = InsertHeaderService<S>;

        fn wrap(&self, service: S) -> InsertHeaderService<S> {
            InsertHeaderService {
                key: self.key.clone(),
                value: self.value.clone(),
                inner: service,
            }
        }
    }

    pub struct InsertHeaderService<T> {
        inner: T,
        key: String,
        value: String,
    }

    impl<T> Service for InsertHeaderService<T>
    where
        T: Service,
        T::Request: Request,
    {
        type Request = T::Request;
        type Response = T::Response;
        type Error = T::Error;
        type Future = T::Future;

        fn poll_ready(&mut self) -> Poll<(), Self::Error> {
            self.inner.poll_ready()
        }

        fn call(&mut self, mut req: Self::Request) -> Self::Future {
            req.context().add_request_header(&*self.key, &*self.value);
            self.inner.call(req)
        }
    }
}
