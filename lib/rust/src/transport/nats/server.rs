use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam::channel;
use nats;

use super::{build_client, NATS_MAX_MESSAGE_SIZE};
use buffer::FMemoryOutputBuffer;
use processor::FProcessor;
use protocol::{FInputProtocolFactory, FOutputProtocolFactory};

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

    pub fn build(self) -> FNatsServer<P> {
        FNatsServer {
            server: self.server,
            tls_config: self.tls_config,
            processor: self.processor,
            iprot_factory: self.iprot_factory,
            oprot_factory: self.oprot_factory,
            subjects: self.subjects,
            queue: self.queue,
            worker_count: self.worker_count,
            queue_len: self.queue_len,
            high_water_mark: self.high_water_mark,
        }
    }
}

// TODO: Impl drop and put the unsub stuff in there. Actually, not sure if that works if serve()
// blocks indefinitely.
pub struct FNatsServer<P>
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

impl<P> FNatsServer<P>
where
    P: FProcessor,
{
    pub fn serve(&mut self) -> thrift::Result<()> {
        let mut sub_client = build_client(&self.server, self.tls_config.clone())?;
        let pub_client = Arc::new(Mutex::new(build_client(
            &self.server,
            self.tls_config.clone(),
        )?));

        let (sender, receiver) = channel::bounded(self.queue_len);

        for subject in self.subjects.iter() {
            sub_client
                .subscribe(&subject, self.queue.as_ref().map(String::as_str))
                .map_err(|err| {
                    thrift::new_transport_error(
                        thrift::TransportErrorKind::Unknown,
                        err.to_string(),
                    )
                })?;
        }

        for _ in 0..self.worker_count {
            let pub_client_clone = pub_client.clone();
            let recv_clone = receiver.clone();
            let high_water_mark_clone = self.high_water_mark;
            let proc_clone = self.processor.clone();
            let iprot_factory_clone = self.iprot_factory.clone();
            let oprot_factory_clone = self.oprot_factory.clone();
            thread::spawn(move || {
                worker(
                    pub_client_clone,
                    recv_clone,
                    high_water_mark_clone,
                    proc_clone,
                    iprot_factory_clone,
                    oprot_factory_clone,
                )
            });
        }

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

        Ok(())
    }
}

fn worker<P>(
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
        let msg = match receiver.recv() {
            Ok(msg) => {
                let dur = msg.timestamp.elapsed();
                if dur > high_water_mark {
                    let millis = dur.as_secs() * 1000 + u64::from(dur.subsec_millis());
                    warn!("frugal: request spent {} in the transport buffer, your consumer might be backed up", millis);
                };
                msg
            }
            Err(_) => {
                error!("frugal: work channel was disconnected, stopping worker");
                break;
            }
        };

        if let Err(err) = process_frame(
            msg,
            &client_mu,
            &mut processor,
            &iprot_factory,
            &oprot_factory,
        ) {
            error!("frugal: error processing request: {}", err);
            continue;
        }
    }
}

fn process_frame<P>(
    msg: FrameWrapper,
    client: &Arc<Mutex<nats::Client>>,
    processor: &mut P,
    iprot_factory: &FInputProtocolFactory,
    oprot_factory: &FOutputProtocolFactory,
) -> thrift::Result<()>
where
    P: FProcessor,
{
    let input = Cursor::new(&msg.frame[4..]);
    let mut output = FMemoryOutputBuffer::new(NATS_MAX_MESSAGE_SIZE);
    let mut iprot = iprot_factory.get_protocol(input);

    {
        let mut oprot = oprot_factory.get_protocol(&mut output);
        processor.process(&mut iprot, &mut oprot)?;
    };

    if output.bytes().is_empty() {
        return Ok(());
    }

    println!("process_frame output: {:?}", output.bytes());
    println!("reply: {}", &msg.reply);

    client
        .lock()
        .unwrap()
        .publish(&msg.reply, output.bytes())
        .map_err(|err| {
            thrift::new_transport_error(thrift::TransportErrorKind::Unknown, err.to_string())
        })
}
