pub mod buffer;
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
