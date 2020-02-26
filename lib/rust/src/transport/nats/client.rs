use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::thread;

use async_trait::async_trait;
use crossbeam::{channel, select};
use log::warn;
use nats::{Client, TlsConfig};
use thrift;
use tokio::task;

use super::{build_client, map_tokio_error, map_tokio_error_string};
use crate::context::{FContext, OP_ID_HEADER};
use crate::protocol::ProtocolMarshaler;
use crate::transport::FTransport;

// This has been interesting to work on b/c its combining blocking requests, between the mutex and
// the blocking I/O of the nats client. I've been using tokio::task::spawn_blocking to allow the
// async runtime to keep progressing, but that means a lot of things have to be cloned (in an Arc
// for immutable things) or outright copied (the payload currently). If we get or convert the nats
// library to async, then most of the task::spawn_blocking can be dropped.

pub const NATS_MAX_MESSAGE_SIZE: usize = 1024 * 1024;

struct Info {
    subject: String,
    inbox: String,
}

#[derive(Clone)]
pub struct FNatsTransport {
    info: Arc<Info>,
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
            info: Arc::new(Info {
                subject: subject.into(),
                inbox,
            }),
            client: Arc::new(Mutex::new(client2)),
            registry,
        })
    }
}

#[async_trait]
impl FTransport for FNatsTransport {
    type Response = Cursor<Vec<u8>>;

    async fn oneway(&self, _ctx: &FContext, payload: &[u8]) -> thrift::Result<()> {
        if payload.len() == 4 {
            return Ok(());
        };

        check_message_size(payload)?;

        let client = self.client.clone();
        let info = self.info.clone();
        let payload_clone = copy_payload(payload);
        task::spawn_blocking(move || {
            client
                .lock()
                .unwrap()
                .publish_with_inbox(&info.subject, &payload_clone, &info.inbox)
                .map_err(map_tokio_error)
        })
        .await
        .map_or_else(|err| Err(map_tokio_error(err)), |r| r)
    }

    async fn request(&self, ctx: &FContext, payload: &[u8]) -> thrift::Result<Cursor<Vec<u8>>> {
        if payload.len() == 4 {
            return Ok(Cursor::new(vec![]));
        };

        let (sender, receiver) = channel::bounded(1);
        let op_id_res = ctx.get_op_id();
        let registry = self.registry.clone();
        task::spawn_blocking(move || {
            let mut registry = registry.lock().unwrap();
            match op_id_res {
                Ok(op_id) => {
                    if registry.contains_key(&op_id) {
                        return Err(thrift::new_transport_error(thrift::TransportErrorKind::Unknown, format!("frugal: context already registered, opid {} is in-flight for another request", &op_id)));
                    } else {
                        registry.insert(op_id, sender);
                    }
                }
                Err(err) => {
                    return Err(map_tokio_error_string(format!("frugal: could not retrieve op id from ctx: {}", err)))
                }
            };
            Ok(())
        }).await.map_or_else(|err| Err(map_tokio_error_string(format!(
                "frugal: error while updating registry: {}",
                err
            ))), |r| r)?;

        check_message_size(payload)?;

        let client = self.client.clone();
        let info = self.info.clone();
        let payload_clone = copy_payload(payload);
        task::spawn_blocking(move || {
            client
                .lock()
                .unwrap()
                .publish_with_inbox(&info.subject, &payload_clone, &info.inbox)
                .map_err(map_tokio_error)
        })
        .await
        .map_or_else(
            |err| {
                Err(map_tokio_error_string(format!(
                    "frugal: error while publishing payload: {}",
                    err
                )))
            },
            |r| r,
        )?;

        let timeout = ctx.timeout();
        task::spawn_blocking(move || {
            select! {
                recv(receiver) -> result => result.map(Cursor::new).map_err(|_| thrift::new_transport_error(thrift::TransportErrorKind::Unknown, "frugal: result channel closed")),
                recv(channel::after(timeout)) -> _ => Err(thrift::new_transport_error(thrift::TransportErrorKind::TimedOut, "frugal: nats request timed out")),
            }
        }).await.map_or_else(|err| Err(map_tokio_error_string(format!(
                "frugal: error while waiting for response: {}",
                err
            ))), |r| r)
    }

    fn get_request_size_limit(&self) -> Option<usize> {
        Some(NATS_MAX_MESSAGE_SIZE)
    }
}

fn copy_payload(payload: &[u8]) -> Vec<u8> {
    let mut v = vec![];
    v.extend_from_slice(payload);
    v
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
