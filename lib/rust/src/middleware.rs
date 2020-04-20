use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use thrift;

use crate::context::FContext;

pub struct Request {
    pub ctx: FContext,

    /// The Arg struct for the method being called.
    pub args: Box<dyn Any + Send>,
}

// TODO: this type doesn't quite feel right yet
pub struct Response {
    /// The Result value for the method being called.
    pub result: thrift::Result<Box<dyn Any + Send>>,
}

#[async_trait]
pub trait Method {
    async fn run(&self, req: Request) -> Response;
}

#[async_trait]
pub trait Middleware: 'static + Send + Sync {
    async fn handle<'a>(&'a self, req: Request, next: Next<'a>) -> Response;
}

pub struct Next<'a> {
    // TODO: Encapsulate these better
    pub method: &'a (dyn Method + Sync),
    pub next_middleware: &'a [Arc<dyn Middleware>],
}

impl<'a> Next<'a> {
    pub async fn run(mut self, req: Request) -> Response {
        match self.next_middleware {
            [] => self.method.run(req).await,
            [current, next @ ..] => {
                self.next_middleware = next;
                current.handle(req, self).await
            }
        }
    }
}

#[cfg(test)]
mod test {
    use async_trait::async_trait;
    use thrift;
    use tokio;

    use super::*;
    use crate::context::FContext;

    #[derive(Debug, PartialEq)]
    struct Argument {
        i: i64,
    }

    fn inner_processor_fn(ctx: FContext, a: String, b: Argument) -> thrift::Result<MyResult> {
        assert_eq!("wtf", ctx.correlation_id());
        assert_eq!("Hello, world! Bob!", &a);
        assert_eq!(8i64, b.i);
        Ok(MyResult(42))
    }

    struct MyResult(i64);

    struct MyArguments {
        a: String,
        b: Argument,
    }

    struct Adapter;

    #[async_trait]
    impl Method for Adapter {
        async fn run(&self, req: Request) -> Response {
            let ctx = req.ctx;
            let args = req
                .args
                .downcast::<MyArguments>()
                .expect("Expected a MyAruments");
            let res = inner_processor_fn(ctx, args.a, args.b)
                .map(|ok| Box::new(ok) as Box<dyn Any + Send>);
            Response { result: res }
        }
    }

    struct MiddlewareA;
    #[async_trait]
    impl Middleware for MiddlewareA {
        async fn handle<'a>(&'a self, mut req: Request, next: Next<'a>) -> Response {
            // adds " Bob!" to a
            let args = req
                .args
                .downcast_mut::<MyArguments>()
                .expect("Expected a MyAruments");
            let new_a = format!("{} Bob!", args.a);
            args.a = new_a;
            next.run(req).await
        }
    }

    struct MiddlewareB;
    #[async_trait]
    impl Middleware for MiddlewareB {
        async fn handle<'a>(&'a self, mut req: Request, next: Next<'a>) -> Response {
            // adds 1 to b.i
            let args = req
                .args
                .downcast_mut::<MyArguments>()
                .expect("Expected a MyAruments");
            args.b.i += 1;
            next.run(req).await
        }
    }

    #[tokio::test]
    async fn test_middleware() {
        let ma = MiddlewareA;
        let mb = MiddlewareB;
        let fctx = FContext::new(Some("wtf"));
        let a = "Hello, world!".to_string();
        let b = Argument { i: 7 };
        let req = Request {
            ctx: fctx,
            args: Box::new(MyArguments { a, b }),
        };
        let adapter = Adapter;
        let middleware_stack = vec![
            Arc::new(ma) as Arc<dyn Middleware>,
            Arc::new(mb) as Arc<dyn Middleware>,
        ];

        let next = Next {
            method: &adapter,
            next_middleware: &middleware_stack,
        };

        let res = next.run(req).await;
        let output_myresult = res.result.unwrap();
        let actual_res = output_myresult
            .downcast_ref::<MyResult>()
            .expect("expected a MyResult");
        assert_eq!(42i64, actual_res.0);
    }
}
