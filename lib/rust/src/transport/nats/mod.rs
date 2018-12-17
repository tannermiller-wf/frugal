use nats::{Client, TlsConfig};
use thrift;

mod client;
mod server;

pub use self::client::*;
pub use self::server::*;

fn build_client(server: &str, tls_config: Option<TlsConfig>) -> thrift::Result<Client> {
    let mut client = Client::new(server).map_err(|err| {
        thrift::new_transport_error(thrift::TransportErrorKind::Unknown, err.to_string())
    })?;
    if let Some(ref tls_config) = tls_config {
        client.set_tls_config(tls_config.clone());
    };
    Ok(client)
}
