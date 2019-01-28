use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::thread;

use crossbeam::channel;
use nats::{Client, TlsConfig};
use thrift;

use super::build_client;
use context::{FContext, OP_ID_HEADER};
use protocol::ProtocolMarshaler;
use transport::FTransport;

pub const NATS_MAX_MESSAGE_SIZE: usize = 1024 * 1024;

pub struct FNatsTransport {
    subject: String,
    inbox: String,
    client: Arc<Mutex<Client>>,
    registry: Arc<Mutex<HashMap<u64, channel::Sender<Vec<u8>>>>>,
}

impl FNatsTransport {
    pub fn new<S1, S2, S3>(
        server: S1,
        tls_config: Option<TlsConfig>,
        subject: S2,
        inbox: S3,
    ) -> thrift::Result<FNatsTransport>
    where
        S1: Into<String>,
        S2: Into<String>,
        S3: Into<String>,
    {
        let server = server.into();
        let mut client1 = build_client(&server, tls_config.clone())?;
        let client2 = build_client(&server, tls_config)?;
        let inbox = inbox.into();
        client1.subscribe(&inbox, None).map_err(|err| {
            thrift::new_transport_error(thrift::TransportErrorKind::Unknown, err.to_string())
        })?;
        let registry: Arc<Mutex<HashMap<u64, channel::Sender<Vec<u8>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let registry_thread = registry.clone();
        thread::spawn(move || {
            for event in client1.events() {
                let nats::Event { msg, .. } = event;

                if msg.len() < 5 {
                    warn!("frugal: invalid protocol frame headers: invalid frame size 0");
                    continue;
                };

                let msg_slice = &msg[4..]; // There are 4 bytes at the beginning that we always ignore

                let prot_marshaler = match ProtocolMarshaler::get(msg_slice[0]) {
                    Ok(prot_marshaler) => prot_marshaler,
                    Err(err) => {
                        warn!("frugal: invalid protocol frame headers: {}", err);
                        continue;
                    }
                };

                let op_id_res = {
                    match prot_marshaler.unmarshal_headers(&mut Cursor::new(&msg_slice[1..])) {
                        Ok(header) => header
                            .get(OP_ID_HEADER)
                            .ok_or_else(|| {
                                format!("frugal: headers did not include {}", OP_ID_HEADER)
                            })
                            .and_then(|op_id_str| {
                                op_id_str.parse().map_err(|err| {
                                    format!(
                                        "frugal: invalid protocol frame, op id not a uint6: {}",
                                        err
                                    )
                                })
                            }),
                        Err(err) => {
                            warn!("frugal: invalid protocol frame headers: {}", err);
                            continue;
                        }
                    }
                };

                match op_id_res {
                    Ok(op_id) => {
                        let registry = registry_thread.lock().unwrap();
                        match registry.get(&op_id) {
                            Some(result_chan) => {
                                if result_chan.try_send(msg_slice.to_vec()).is_err() {
                                    warn!("frugal: op id already handled");
                                    continue;
                                }
                            }
                            None => {
                                warn!("frugal: unregistered context");
                                continue;
                            }
                        };
                    }
                    Err(err) => {
                        warn!(
                            "frugal: invalid protocol frame, op id not a uint64: {}",
                            err
                        );
                        continue;
                    }
                }
            }
        });
        Ok(FNatsTransport {
            subject: subject.into(),
            inbox,
            client: Arc::new(Mutex::new(client2)),
            registry,
        })
    }
}

impl<'a> FTransport for FNatsTransport {
    type Response = Cursor<Vec<u8>>;

    fn oneway(&mut self, _ctx: &FContext, payload: &[u8]) -> thrift::Result<()> {
        if payload.len() == 4 {
            return Ok(());
        };

        check_message_size(payload)?;

        self.client
            .lock()
            .unwrap()
            .publish_with_inbox(&self.subject, payload, &self.inbox)
            .map_err(|err| {
                thrift::new_transport_error(thrift::TransportErrorKind::Unknown, err.to_string())
            })
    }

    fn request(&mut self, ctx: &FContext, payload: &[u8]) -> thrift::Result<Cursor<Vec<u8>>> {
        if payload.len() == 4 {
            return Ok(Cursor::new(vec![]));
        };

        let (sender, receiver) = channel::bounded(1);
        let op_id_res = ctx.get_op_id();
        {
            let mut registry = self.registry.lock().unwrap();
            match op_id_res {
                Ok(op_id) => {
                    if registry.contains_key(&op_id) {
                        return Err(thrift::new_transport_error(thrift::TransportErrorKind::Unknown, format!("frugal: context already registered, opid {} is in-flight for another request", &op_id)));
                    } else {
                        registry.insert(op_id, sender);
                    }
                }
                Err(err) => {
                    return Err(thrift::new_transport_error(
                        thrift::TransportErrorKind::Unknown,
                        format!("frugal: could not retrieve op id from ctx: {}", err),
                    ))
                }
            };
        };

        check_message_size(payload)?;

        self.client
            .lock()
            .unwrap()
            .publish_with_inbox(&self.subject, payload, &self.inbox)
            .map_err(|err| {
                thrift::new_transport_error(thrift::TransportErrorKind::Unknown, err.to_string())
            })?;

        select! {
            recv(receiver) -> result => result.map(Cursor::new).map_err(|_| thrift::new_transport_error(thrift::TransportErrorKind::Unknown, "frugal: result channel closed")),
            recv(channel::after(ctx.timeout())) -> _ => Err(thrift::new_transport_error(thrift::TransportErrorKind::TimedOut, "frugal: nats request timed out")),
        }
    }

    fn get_request_size_limit(&self) -> Option<usize> {
        Some(NATS_MAX_MESSAGE_SIZE)
    }
}

fn check_message_size(data: &[u8]) -> thrift::Result<()> {
    if data.len() > NATS_MAX_MESSAGE_SIZE {
        Err(thrift::new_transport_error(
            thrift::TransportErrorKind::SizeLimit,
            format!(
                "Message exceeds {} bytes, was {} bytes",
                NATS_MAX_MESSAGE_SIZE,
                data.len()
            ),
        ))
    } else {
        Ok(())
    }
}