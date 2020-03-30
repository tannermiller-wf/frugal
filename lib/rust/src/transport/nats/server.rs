use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam::channel;
use lazy_static::lazy_static;
use log::{error, warn};
use nats;
use tokio::task;

use super::{build_client, map_tokio_error, NATS_MAX_MESSAGE_SIZE};
use crate::buffer::FMemoryOutputBuffer;
use crate::processor::FProcessor;
use crate::protocol::{FInputProtocolFactory, FOutputProtocolFactory};

const DEFAULT_WORK_QUEUE_LEN: usize = 64;

lazy_static! {
    static ref DEFAULT_WATER_MARK: Duration = Duration::from_secs(5);
}

struct FrameWrapper {
    frame: Vec<u8>,
    timestamp: Instant,
    reply: String,
}

pub struct FNatsServerBuilder<P>
where
    P: FProcessor,
{
    server: String,
    tls_config: Option<nats::TlsConfig>,
    processor: P,
    iprot_factory: FInputProtocolFactory,
    oprot_factory: FOutputProtocolFactory,
    subjects: Vec<String>,
    queue: Option<String>,
    worker_count: usize,
    queue_len: usize,
    high_water_mark: Duration,
}

impl<P> FNatsServerBuilder<P>
where
    P: FProcessor,
{
    pub fn new<S: Into<String>>(
        server: S,
        processor: P,
        iprot_factory: FInputProtocolFactory,
        oprot_factory: FOutputProtocolFactory,
        subjects: Vec<String>,
    ) -> FNatsServerBuilder<P> {
        FNatsServerBuilder {
            server: server.into(),
            tls_config: None,
            processor,
            iprot_factory,
            oprot_factory,
            subjects,
            queue: None,
            worker_count: 1,
            queue_len: DEFAULT_WORK_QUEUE_LEN,
            high_water_mark: *DEFAULT_WATER_MARK,
        }
    }

    pub fn with_tls_config(mut self, tls_config: nats::TlsConfig) -> Self {
        self.tls_config = Some(tls_config);
        self
    }

    pub fn with_queue_group<S: Into<String>>(mut self, queue: S) -> Self {
        self.queue = Some(queue.into());
        self
    }

    pub fn with_worker_count(mut self, worker_count: usize) -> Self {
        self.worker_count = worker_count;
        self
    }

    pub fn with_queue_length(mut self, queue_len: usize) -> Self {
        self.queue_len = queue_len;
        self
    }

    pub fn with_high_water_mark(mut self, high_water_mark: Duration) -> Self {
        self.high_water_mark = high_water_mark;
        self
    }

    pub async fn serve(self) -> thrift::Result<()> {
        let mut sub_client = build_client(&self.server, self.tls_config.clone())?;
        let pub_client = Arc::new(Mutex::new(build_client(
            &self.server,
            self.tls_config.clone(),
        )?));

        let (sender, receiver) = channel::bounded(self.queue_len);

        // blocking in place b/c we can't move the sub_client yet
        task::block_in_place::<_, thrift::Result<()>>(|| {
            for subject in self.subjects.iter() {
                sub_client
                    .subscribe(&subject, self.queue.as_deref())
                    .map_err(map_tokio_error)?;
            }
            Ok(())
        })?;

        for _ in 0..self.worker_count {
            let pub_client_clone = pub_client.clone();
            let recv_clone = receiver.clone();
            let high_water_mark_clone = self.high_water_mark;
            let proc_clone = self.processor.clone();
            let iprot_factory_clone = self.iprot_factory.clone();
            let oprot_factory_clone = self.oprot_factory.clone();
            task::spawn(worker(
                pub_client_clone,
                recv_clone,
                high_water_mark_clone,
                proc_clone,
                iprot_factory_clone,
                oprot_factory_clone,
            ));
        }

        // now move this to the blocking executor as there's no more async in it
        task::spawn_blocking(move || {
            for msg in sub_client.events() {
                let reply = match msg.inbox {
                    Some(reply) => reply,
                    None => {
                        warn!("frugal: discarding invalid NATS request (no reply)");
                        continue;
                    }
                };

                let send_result = sender.send(FrameWrapper {
                    frame: msg.msg,
                    timestamp: Instant::now(),
                    reply,
                });

                if send_result.is_err() {
                    error!("frugal: work channel was disconnected, stopping subscription");
                    break;
                };
            }
        })
        .await
        .map_err(map_tokio_error)
    }
}

async fn worker<P>(
    client_mu: Arc<Mutex<nats::Client>>,
    receiver: channel::Receiver<FrameWrapper>,
    high_water_mark: Duration,
    mut processor: P,
    iprot_factory: FInputProtocolFactory,
    oprot_factory: FOutputProtocolFactory,
) where
    P: FProcessor,
{
    loop {
        // must move the receiver to another thread, so clone it first
        let r = receiver.clone();
        let msg = match task::spawn_blocking(move || r.recv()).await {
            Ok(Ok(msg)) => {
                let dur = msg.timestamp.elapsed();
                if dur > high_water_mark {
                    let millis = dur.as_secs() * 1000 + u64::from(dur.subsec_millis());
                    warn!("frugal: request spent {} in the transport buffer, your consumer might be backed up", millis);
                };
                msg
            }
            _ => {
                error!("frugal: work channel was disconnected, stopping worker");
                break;
            }
        };

        // TODO: process_frame will probably be async too
        let output = match process_frame(&msg.frame, &mut processor, &iprot_factory, &oprot_factory)
        {
            Ok(output) => output,
            Err(err) => {
                error!("frugal: error processing request: {}", err);
                continue;
            }
        };

        let client = client_mu.clone();
        let res = task::spawn_blocking(move || {
            client
                .lock()
                .unwrap()
                .publish(&msg.reply, output.bytes())
                .map_err(|err| {
                    thrift::new_transport_error(
                        thrift::TransportErrorKind::Unknown,
                        err.to_string(),
                    )
                })
        })
        .await;
        match res {
            Ok(Ok(())) => (),
            Ok(Err(err)) => error!("frugal: error processing request: {}", err),
            Err(err) => error!("frugal: error processing request: {}", err),
        };
    }
}

fn process_frame<P>(
    frame: &[u8],
    processor: &mut P,
    iprot_factory: &FInputProtocolFactory,
    oprot_factory: &FOutputProtocolFactory,
) -> thrift::Result<FMemoryOutputBuffer>
where
    P: FProcessor,
{
    let input = Cursor::new(&frame[4..]);
    let mut output = FMemoryOutputBuffer::new(NATS_MAX_MESSAGE_SIZE);
    let mut iprot = iprot_factory.get_protocol(input);

    {
        let mut oprot = oprot_factory.get_protocol(&mut output);
        // TODO: processor will probably be async
        processor.process(&mut iprot, &mut oprot)?;
    };

    Ok(output)
}
