use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;

use crate::context::FContext;

// TODO: Maybe the args should be generic and it is the F*Args struct generated for each request?
pub struct Request {
    pub ctx: FContext,
    pub args: Vec<Box<dyn Any + Send>>,
}

// TODO: This probably has some fields on it, Or is generic and generated F*Result struct for each
// request
pub struct Response;

#[async_trait]
pub trait FProcessorAdapter {
    async fn run(&self, req: Request) -> Response;
}

pub struct Next<'a> {
    processor: &'a (dyn FProcessorAdapter + Sync),
    next_middleware: &'a [Arc<dyn Middleware>],
}

impl<'a> Next<'a> {
    pub async fn run(mut self, req: Request) -> Response {
        match self.next_middleware {
            [] => self.processor.run(req).await,
            [current, next @ ..] => {
                self.next_middleware = next;
                current.handle(req, self).await
            }
        }
    }
}

#[async_trait]
pub trait Middleware: 'static + Send + Sync {
    async fn handle<'a>(&'a self, req: Request, next: Next<'a>) -> Response;
}

#[cfg(test)]
mod test {
    use async_trait::async_trait;
    use tokio;

    use super::*;
    use crate::context::FContext;

    #[derive(Debug, PartialEq)]
    struct Argument {
        i: i64,
    }

    fn inner_processor_fn(ctx: FContext, a: String, b: Argument) {
        assert_eq!("wtf", ctx.correlation_id());
        assert_eq!("Hello, world! Bob!", &a);
        assert_eq!(8i64, b.i);
    }

    struct Adapter;

    #[async_trait]
    impl FProcessorAdapter for Adapter {
        async fn run(&self, mut req: Request) -> Response {
            if req.args.len() != 2 {
                panic!("expected 2 args")
            }
            let ctx = req.ctx;
            let b = req
                .args
                .pop()
                .unwrap()
                .downcast::<Argument>()
                .expect("expected an Argument for b");
            let a = req
                .args
                .pop()
                .unwrap()
                .downcast::<String>()
                .expect("expected a String for a");
            inner_processor_fn(ctx, *a, *b);
            Response
        }
    }

    struct MiddlewareA;
    #[async_trait]
    impl Middleware for MiddlewareA {
        async fn handle<'a>(&'a self, mut req: Request, next: Next<'a>) -> Response {
            // adds " Bob!" to a
            let a = req.args[0]
                .downcast_ref::<String>()
                .expect("expected a string for a");
            let new_a = format!("{} Bob!", a);
            let new_req = Request {
                ctx: req.ctx,
                args: vec![Box::new(new_a), req.args.pop().unwrap()],
            };
            next.run(new_req).await
        }
    }

    struct MiddlewareB;
    #[async_trait]
    impl Middleware for MiddlewareB {
        async fn handle<'a>(&'a self, mut req: Request, next: Next<'a>) -> Response {
            // adds 1 to b.i
            let b = req.args[1]
                .downcast_mut::<Argument>()
                .expect("expected an Argument for b");
            b.i += 1;
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
            args: vec![Box::new(a), Box::new(b)],
        };
        let adapter = Adapter;
        let middleware_stack = vec![
            Arc::new(ma) as Arc<dyn Middleware>,
            Arc::new(mb) as Arc<dyn Middleware>,
        ];

        let next = Next {
            processor: &adapter,
            next_middleware: &middleware_stack,
        };

        next.run(req).await;
    }
}
