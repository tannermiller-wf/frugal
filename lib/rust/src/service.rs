use crate::context::FContext;

pub trait Request {
    fn context(&mut self) -> &mut FContext;
    fn method_name(&self) -> &'static str; // TODO: Is static here fine? Would it ever change?
}
