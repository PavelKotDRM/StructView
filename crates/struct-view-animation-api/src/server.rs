use serde_json::{Value, json};
use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{self, Receiver, SyncSender, TryRecvError},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_BODY: usize = 65_536;
const MAX_HEADERS: usize = 8192;
const TIMEOUT: Duration = Duration::from_secs(2);
const MAX_CLIENTS: usize = 8;

mod config;
mod protocol;
mod transport;

use config::{host_text, load_tls, validate_host};
use protocol::{serve, write_reply};
use transport::Connection;

pub struct TlsFiles {
    pub certificate: PathBuf,
    pub private_key: PathBuf,
}

pub struct ServerOptions {
    pub bind: IpAddr,
    pub port: u16,
    pub host: Option<String>,
    pub tls: Option<TlsFiles>,
    pub allow_network: bool,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            bind: Ipv4Addr::LOCALHOST.into(),
            port: 0,
            host: None,
            tls: None,
            allow_network: false,
        }
    }
}

pub enum ApiOperation {
    Capabilities,
    Graph,
    Status,
    Play(Value),
    Realtime(Value),
    RealtimeStatus,
    Jobs(Value),
    JobsStatus,
    Stop,
}

impl ApiOperation {
    pub(super) fn endpoint(&self) -> &'static str {
        match self {
            Self::Capabilities => "GET /v1/capabilities",
            Self::Graph => "GET /v1/graph",
            Self::Status => "GET /v1/animation",
            Self::Play(_) => "POST /v1/animation",
            Self::Realtime(_) => "POST /v1/realtime",
            Self::RealtimeStatus => "GET /v1/realtime",
            Self::Jobs(_) => "POST /v1/jobs",
            Self::JobsStatus => "GET /v1/jobs",
            Self::Stop => "POST /v1/animation/stop",
        }
    }

    pub(super) fn summary(&self) -> String {
        match self {
            Self::Play(value) => format!(
                "steps: {}",
                value
                    .get("steps")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len)
            ),
            Self::Realtime(value) | Self::Jobs(value) => format!(
                "commands: {}",
                value
                    .get("commands")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len)
            ),
            _ => String::new(),
        }
    }
}

pub struct ApiReply {
    pub status: u16,
    pub value: Value,
}

pub struct ApiRequest {
    pub operation: ApiOperation,
    pub reply: SyncSender<ApiReply>,
    pub deadline: Instant,
}

/// Bounded metadata for one completed HTTP request; payloads and credentials are omitted.
#[derive(Clone, Debug)]
pub struct ApiRequestRecord {
    pub endpoint: String,
    pub summary: String,
    pub status: u16,
    pub duration_ms: u128,
}

/// Live transport counters and recent request metadata.
#[derive(Clone, Debug)]
pub struct ApiServerStats {
    pub active_connections: usize,
    pub queued_requests: usize,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub recent_requests: Vec<ApiRequestRecord>,
}

struct ApiTelemetry {
    active_connections: AtomicUsize,
    queued_requests: AtomicUsize,
    successful_requests: AtomicU64,
    failed_requests: AtomicU64,
    recent_requests: Mutex<VecDeque<ApiRequestRecord>>,
}

impl ApiTelemetry {
    fn new() -> Self {
        Self {
            active_connections: AtomicUsize::new(0),
            queued_requests: AtomicUsize::new(0),
            successful_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            recent_requests: Mutex::new(VecDeque::with_capacity(50)),
        }
    }

    fn record(&self, endpoint: &str, summary: &str, status: u16, duration: Duration) {
        if status < 400 {
            self.successful_requests.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }
        if let Ok(mut recent) = self.recent_requests.lock() {
            if recent.len() == 50 {
                recent.pop_front();
            }
            recent.push_back(ApiRequestRecord {
                endpoint: endpoint.to_owned(),
                summary: summary.to_owned(),
                status,
                duration_ms: duration.as_millis(),
            });
        } else {
            eprintln!("Animation API request history lock poisoned");
        }
    }

    fn snapshot(&self) -> Result<ApiServerStats, String> {
        let recent_requests = self
            .recent_requests
            .lock()
            .map_err(|_| "Animation API request history lock poisoned".to_string())?
            .iter()
            .cloned()
            .rev()
            .collect();
        Ok(ApiServerStats {
            active_connections: self.active_connections.load(Ordering::Relaxed),
            queued_requests: self.queued_requests.load(Ordering::Relaxed),
            successful_requests: self.successful_requests.load(Ordering::Relaxed),
            failed_requests: self.failed_requests.load(Ordering::Relaxed),
            recent_requests,
        })
    }
}

/// Explicitly enabled server. Dropping it stops all workers and invalidates the token.
pub struct AnimationApiServer {
    address: SocketAddr,
    url: String,
    token: String,
    secret: Arc<RwLock<String>>,
    requests: Receiver<ApiRequest>,
    telemetry: Arc<ApiTelemetry>,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl AnimationApiServer {
    pub fn start(port: u16) -> Result<Self, String> {
        Self::start_with(ServerOptions {
            port,
            ..Default::default()
        })
    }

    pub fn start_with(options: ServerOptions) -> Result<Self, String> {
        Self::start_with_waker(options, || {})
    }

    pub fn start_with_waker(
        options: ServerOptions,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        if !options.bind.is_loopback() && !options.allow_network {
            return Err(
                "Network access must be explicitly enabled for a non-loopback bind address".into(),
            );
        }
        let tls = options
            .tls
            .as_ref()
            .map(load_tls)
            .transpose()?
            .map(Arc::new);
        let public_host = match options.host.as_deref().filter(|host| !host.is_empty()) {
            Some(host) => validate_host(host)?,
            None if options.bind.is_unspecified() => {
                return Err(
                    "Specify the public IP or DNS host when listening on all interfaces".into(),
                );
            }
            None => host_text(options.bind),
        };
        let listener = TcpListener::bind((options.bind, options.port))
            .map_err(|error| format!("Cannot start animation API: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let address = listener.local_addr().map_err(|error| error.to_string())?;
        let authority = format!("{public_host}:{}", address.port());
        let url = format!(
            "{}://{authority}",
            if tls.is_some() { "https" } else { "http" }
        );
        let mut hosts = vec![authority];
        if address.port() == if tls.is_some() { 443 } else { 80 } {
            hosts.push(public_host);
        }
        if !address.ip().is_unspecified() {
            hosts.push(address.to_string());
            if address.port() == if tls.is_some() { 443 } else { 80 } {
                hosts.push(host_text(address.ip()));
            }
        }
        if address.ip().is_loopback() {
            hosts.push(format!("localhost:{}", address.port()));
            if address.port() == if tls.is_some() { 443 } else { 80 } {
                hosts.push("localhost".into());
            }
        }
        let hosts = Arc::new(hosts);
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let worker_wake = Arc::clone(&wake);
        let token = Self::generate_token()?;
        let secret = Arc::new(RwLock::new(token.clone()));
        let worker_secret = Arc::clone(&secret);
        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&shutdown);
        let telemetry = Arc::new(ApiTelemetry::new());
        let worker_telemetry = Arc::clone(&telemetry);
        let (sender, requests) = mpsc::sync_channel(16);
        let worker = thread::Builder::new()
            .name("struct-view-animation-api".into())
            .spawn(move || {
                let mut clients: Vec<JoinHandle<()>> = Vec::new();
                while !stop.load(Ordering::Relaxed) {
                    let mut index = 0;
                    while index < clients.len() {
                        if clients[index].is_finished() {
                            if clients.swap_remove(index).join().is_err() {
                                eprintln!("Animation API connection worker panicked");
                            }
                        } else { index += 1; }
                    }
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if let Err(error) = stream.set_nonblocking(false) {
                                eprintln!("Cannot configure animation API client socket: {error}");
                                continue;
                            }
                            if clients.len() >= MAX_CLIENTS {
                                eprintln!("Animation API concurrent client limit reached");
                                worker_telemetry
                                    .active_connections
                                    .fetch_add(1, Ordering::Relaxed);
                                let _client = ActiveClient(Arc::clone(&worker_telemetry));
                                let result = Connection::new(stream, tls.clone()).and_then(
                                    |mut connection| {
                                        write_reply(
                                            &mut connection,
                                            ApiReply {
                                                status: 503,
                                                value: json!({"error":"Too many concurrent API clients"}),
                                            },
                                        )
                                    },
                                );
                                worker_telemetry.record(
                                    "Connection limit",
                                    "",
                                    503,
                                    Duration::ZERO,
                                );
                                if let Err(error) = result {
                                    eprintln!("Animation API rejection error: {error}");
                                }
                                continue;
                            }
                            let tls = tls.clone();
                            let secret = Arc::clone(&worker_secret);
                            let hosts = Arc::clone(&hosts);
                            let sender = sender.clone();
                            let wake = Arc::clone(&wake);
                            let shutdown = Arc::clone(&stop);
                            let telemetry = Arc::clone(&worker_telemetry);
                            telemetry.active_connections.fetch_add(1, Ordering::Relaxed);
                            match thread::Builder::new().name("struct-view-api-client".into()).spawn(move || {
                                let _client = ActiveClient(telemetry);
                                let result = Connection::new(stream,tls)
                                    .and_then(|mut stream| serve(&mut stream,&secret,&hosts,&sender,wake.as_ref(),&shutdown,&_client.0));
                                // Clients can disconnect at any time; transport errors are local to that connection.
                                if let Err(error) = result { eprintln!("Animation API connection error: {error}"); }
                            }) {
                                Ok(client) => clients.push(client),
                                Err(error) => {
                                    worker_telemetry.active_connections.fetch_sub(1, Ordering::Relaxed);
                                    eprintln!("Cannot start animation API client: {error}");
                                }
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => {
                            eprintln!("Animation API listener error: {error}");
                            break;
                        }
                    }
                }
                stop.store(true,Ordering::Relaxed);
                for client in clients {
                    if client.join().is_err() { eprintln!("Animation API connection worker panicked"); }
                }
                drop(sender);
                worker_wake();
            })
            .map_err(|error| format!("Cannot start animation API worker: {error}"))?;
        Ok(Self {
            address,
            url,
            token,
            secret,
            requests,
            telemetry,
            shutdown,
            worker: Some(worker),
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn set_token(&mut self, token: String) -> Result<(), String> {
        if !(16..=256).contains(&token.len()) || !token.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(
                "Token must contain 16-256 printable ASCII characters without spaces".into(),
            );
        }
        *self.secret.write().map_err(|_| "API token lock poisoned")? = token.clone();
        self.token = token;
        Ok(())
    }

    pub fn regenerate_token(&mut self) -> Result<(), String> {
        self.set_token(Self::generate_token()?)
    }
    pub fn url(&self) -> &str {
        &self.url
    }

    fn generate_token() -> Result<String, String> {
        let mut entropy = [0u8; 32];
        getrandom::fill(&mut entropy)
            .map_err(|error| format!("Cannot generate API token: {error}"))?;
        Ok(entropy.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    pub fn try_request(&self) -> Result<Option<ApiRequest>, String> {
        match self.requests.try_recv() {
            Ok(request) => {
                self.telemetry
                    .queued_requests
                    .fetch_sub(1, Ordering::Relaxed);
                Ok(Some(request))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err("Graph animation API worker stopped unexpectedly".into())
            }
        }
    }

    /// Return a snapshot of connection, queue, and recent request statistics.
    pub fn stats(&self) -> Result<ApiServerStats, String> {
        self.telemetry.snapshot()
    }
}

struct ActiveClient(Arc<ApiTelemetry>);

impl Drop for ActiveClient {
    fn drop(&mut self) {
        self.0.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Drop for AnimationApiServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            eprintln!("Animation API worker panicked");
        }
    }
}
