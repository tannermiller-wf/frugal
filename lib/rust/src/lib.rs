extern crate base64;
extern crate byteorder;
extern crate futures;
#[macro_use]
extern crate hyper;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate mime;
extern crate thrift;
extern crate tokio_core;
extern crate tower_service;
extern crate uuid;

pub mod context;
pub mod errors;
pub mod processor;
pub mod protocol;
pub mod provider;
pub mod service;
pub mod transport;

mod util;

// TODO: Run benchmarks among all the frugal implementations, simple service that pings as hard as it
// can, compare Go, Rust, Python?

// TODO: Consider using the failure crate for errors
