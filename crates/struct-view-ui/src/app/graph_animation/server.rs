use super::{AnimationScenario, RealtimeUpdate};
use serde_json::{Value, json};
use std::io::{BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TryRecvError},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_BODY: usize = 65_536;
const MAX_HEADERS: usize = 8192;
const TIMEOUT: Duration = Duration::from_secs(2);
const MAX_CLIENTS: usize = 8;

pub(super) struct TlsFiles {
    pub certificate: PathBuf,
    pub private_key: PathBuf,
}

pub(super) struct ServerOptions {
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

pub(super) enum ApiOperation {
    Graph,
    Status,
    Play(AnimationScenario),
    Realtime(RealtimeUpdate),
    RealtimeStatus,
    Stop,
}

pub(super) struct ApiReply {
    pub status: u16,
    pub value: Value,
}

pub(super) struct ApiRequest {
    pub operation: ApiOperation,
    pub reply: SyncSender<ApiReply>,
    pub deadline: Instant,
}

/// Explicitly enabled server. Dropping it stops all workers and invalidates the token.
pub(super) struct AnimationApiServer {
    address: SocketAddr,
    url: String,
    token: String,
    secret: Arc<RwLock<String>>,
    requests: Receiver<ApiRequest>,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl AnimationApiServer {
    #[cfg(test)]
    pub fn start(port: u16, ctx: egui::Context) -> Result<Self, String> {
        Self::start_with(
            ServerOptions {
                port,
                ..Default::default()
            },
            ctx,
        )
    }

    pub fn start_with(options: ServerOptions, ctx: egui::Context) -> Result<Self, String> {
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
        let token = Self::generate_token()?;
        let secret = Arc::new(RwLock::new(token.clone()));
        let worker_secret = Arc::clone(&secret);
        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&shutdown);
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
                                if tls.is_none() {
                                    let mut connection = Connection::plain(stream);
                                    if let Err(error) = write_reply(&mut connection,ApiReply {
                                        status:503,value:json!({"error":"Too many concurrent API clients"})
                                    }) {
                                        eprintln!("Animation API rejection error: {error}");
                                    }
                                }
                                continue;
                            }
                            let tls = tls.clone();
                            let secret = Arc::clone(&worker_secret);
                            let hosts = Arc::clone(&hosts);
                            let sender = sender.clone();
                            let ctx = ctx.clone();
                            let shutdown = Arc::clone(&stop);
                            match thread::Builder::new().name("struct-view-api-client".into()).spawn(move || {
                                let result = Connection::new(stream,tls)
                                    .and_then(|mut stream| serve(&mut stream,&secret,&hosts,&sender,&ctx,&shutdown));
                                // Clients can disconnect at any time; transport errors are not GUI failures.
                                if let Err(error) = result { eprintln!("Animation API connection error: {error}"); }
                            }) {
                                Ok(client) => clients.push(client),
                                Err(error) => eprintln!("Cannot start animation API client: {error}"),
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
                ctx.request_repaint();
            })
            .map_err(|error| format!("Cannot start animation API worker: {error}"))?;
        Ok(Self {
            address,
            url,
            token,
            secret,
            requests,
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

    pub(super) fn try_request(&self) -> Result<Option<ApiRequest>, String> {
        match self.requests.try_recv() {
            Ok(request) => Ok(Some(request)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err("Graph animation API worker stopped unexpectedly".into())
            }
        }
    }
}

fn host_text(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    }
}

fn validate_host(host: &str) -> Result<String, String> {
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        if ip.is_unspecified() {
            return Err("Public host must not be an unspecified IP".into());
        }
        return Ok(host_text(ip));
    }
    if host.len() > 253
        || host.is_empty()
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err("Public host must be an IP or DNS name without scheme, port or path".into());
    }
    Ok(host.to_ascii_lowercase())
}

fn load_tls(files: &TlsFiles) -> Result<rustls::ServerConfig, String> {
    let cert = std::fs::File::open(&files.certificate)
        .map_err(|error| format!("Cannot read TLS certificate: {error}"))?;
    let certificates = rustls_pemfile::certs(&mut BufReader::new(cert))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Invalid TLS certificate PEM: {error}"))?;
    if certificates.is_empty() {
        return Err("TLS certificate PEM contains no certificates".into());
    }
    let key = std::fs::File::open(&files.private_key)
        .map_err(|error| format!("Cannot read TLS private key: {error}"))?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(key))
        .map_err(|error| format!("Invalid TLS private key PEM: {error}"))?
        .ok_or("TLS PEM contains no private key")?;
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|error| format!("Cannot configure TLS: {error}"))?
    .with_no_client_auth()
    .with_single_cert(certificates, key)
    .map_err(|error| format!("Invalid TLS certificate/key: {error}"))?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

struct DeadlineSocket {
    stream: TcpStream,
    deadline: Instant,
    write_deadline: Instant,
}
impl Read for DeadlineSocket {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "API read deadline exceeded")
            })?;
        self.stream.set_read_timeout(Some(remaining))?;
        self.stream.read(buffer)
    }
}
impl Write for DeadlineSocket {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let remaining = self
            .write_deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "API write deadline exceeded")
            })?;
        self.stream.set_write_timeout(Some(remaining))?;
        self.stream.write(buffer)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

enum Connection {
    Plain(DeadlineSocket),
    Tls(Box<rustls::StreamOwned<rustls::ServerConnection, DeadlineSocket>>),
}
impl Connection {
    fn plain(stream: TcpStream) -> Self {
        Self::Plain(DeadlineSocket {
            stream,
            deadline: Instant::now() + TIMEOUT,
            write_deadline: Instant::now() + TIMEOUT,
        })
    }
    fn new(stream: TcpStream, config: Option<Arc<rustls::ServerConfig>>) -> std::io::Result<Self> {
        let Some(config) = config else {
            return Ok(Self::plain(stream));
        };
        let connection = rustls::ServerConnection::new(config).map_err(std::io::Error::other)?;
        let mut stream = rustls::StreamOwned::new(
            connection,
            DeadlineSocket {
                stream,
                deadline: Instant::now() + TIMEOUT,
                write_deadline: Instant::now() + TIMEOUT,
            },
        );
        while stream.conn.is_handshaking() {
            stream.conn.complete_io(&mut stream.sock)?;
        }
        Ok(Self::Tls(Box::new(stream)))
    }
}
impl Read for Connection {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(buffer),
            Self::Tls(stream) => stream.read(buffer),
        }
    }
}
impl Write for Connection {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.write(buffer),
            Self::Tls(stream) => stream.write(buffer),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(stream) => stream.flush(),
            Self::Tls(stream) => stream.flush(),
        }
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

fn serve(
    stream: &mut Connection,
    token: &RwLock<String>,
    hosts: &[String],
    sender: &SyncSender<ApiRequest>,
    ctx: &egui::Context,
    shutdown: &AtomicBool,
) -> std::io::Result<()> {
    let parsed = parse_request(stream, token, hosts);
    let operation = match parsed {
        Ok(operation) => operation,
        Err((status, error)) => {
            return write_reply(
                stream,
                ApiReply {
                    status,
                    value: json!({"error":error}),
                },
            );
        }
    };
    let (reply, received) = mpsc::sync_channel(1);
    let deadline = Instant::now() + TIMEOUT;
    if sender
        .try_send(ApiRequest {
            operation,
            reply,
            deadline,
        })
        .is_err()
    {
        return write_reply(
            stream,
            ApiReply {
                status: 503,
                value: json!({"error":"API queue is full or closed"}),
            },
        );
    }
    ctx.request_repaint();
    let response = loop {
        if shutdown.load(Ordering::Relaxed) {
            break ApiReply {
                status: 503,
                value: json!({"error":"API stopped"}),
            };
        }
        match received.recv_timeout(Duration::from_millis(20)) {
            Ok(reply) => break reply,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break ApiReply {
                    status: 503,
                    value: json!({"error":"GUI request channel closed"}),
                };
            }
            Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() >= deadline => {
                break ApiReply {
                    status: 504,
                    value: json!({"error":"GUI response timed out"}),
                };
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    };
    write_reply(stream, response)
}

type ParseResult = Result<ApiOperation, (u16, String)>;

fn parse_request(stream: &mut Connection, token: &RwLock<String>, hosts: &[String]) -> ParseResult {
    let mut reader = BufReader::new(stream);
    let mut consumed = 0;
    let mut line = Vec::new();
    let deadline = Instant::now() + TIMEOUT;
    let mut read_line = |reader: &mut BufReader<&mut Connection>| -> Result<String, (u16, String)> {
        line.clear();
        loop {
            if Instant::now() >= deadline {
                return Err((408, "HTTP request timed out".into()));
            }
            let mut byte = [0];
            reader
                .read_exact(&mut byte)
                .map_err(|error| (408, error.to_string()))?;
            consumed += 1;
            if consumed > MAX_HEADERS {
                return Err((413, "HTTP headers too large".into()));
            }
            line.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        if !line.ends_with(b"\r\n") {
            return Err((400, "Expected HTTP CRLF headers".into()));
        }
        String::from_utf8(line[..line.len() - 2].to_vec())
            .map_err(|_| (400, "Headers must be UTF-8".into()))
    };
    let request_line = read_line(&mut reader)?;
    let parts = request_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" {
        return Err((400, "Expected HTTP/1.1".into()));
    }
    let mut host = None;
    let mut auth = None;
    let mut content_length = None;
    let mut content_type = None;
    loop {
        let header = read_line(&mut reader)?;
        if header.is_empty() {
            break;
        }
        let (name, value) = header
            .split_once(':')
            .ok_or((400, "Invalid HTTP header".into()))?;
        let value = value.trim().to_string();
        match name.to_ascii_lowercase().as_str() {
            "host" => set_header(&mut host, value)?,
            "authorization" => set_header(&mut auth, value)?,
            "content-length" => set_header(&mut content_length, value)?,
            "content-type" => set_header(&mut content_type, value)?,
            "origin" => return Err((403, "Browser-origin requests are not allowed".into())),
            "transfer-encoding" | "expect" => {
                return Err((400, "Chunked requests and Expect are not supported".into()));
            }
            _ => {}
        }
    }
    let token = token
        .read()
        .map_err(|_| (503, "API token lock poisoned".into()))?
        .clone();
    if !host.as_ref().is_some_and(|host| {
        hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host))
    }) {
        return Err((403, "Host must match a configured API address".into()));
    }
    if auth.as_deref() != Some(&format!("Bearer {token}")) {
        return Err((401, "A valid Bearer token is required".into()));
    }
    let length = content_length
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| (400, "Invalid Content-Length".into()))
        })
        .transpose()?
        .unwrap_or(0);
    if length > MAX_BODY {
        return Err((413, "Request body exceeds 65536 bytes".into()));
    }
    match (parts[0], parts[1]) {
        ("GET", "/v1/graph") if length == 0 => Ok(ApiOperation::Graph),
        ("GET", "/v1/animation") if length == 0 => Ok(ApiOperation::Status),
        ("GET", "/v1/realtime") if length == 0 => Ok(ApiOperation::RealtimeStatus),
        ("POST", "/v1/animation/stop") if length == 0 => Ok(ApiOperation::Stop),
        ("POST", path @ ("/v1/animation" | "/v1/realtime")) => {
            if !content_type.as_deref().is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
            }) {
                return Err((415, "Content-Type must be application/json".into()));
            }
            if length == 0 {
                return Err((400, "A JSON scenario body is required".into()));
            }
            let mut body = vec![0u8; length];
            let mut offset = 0;
            while offset < length {
                if Instant::now() >= deadline {
                    return Err((408, "HTTP request timed out".into()));
                }
                let read = reader
                    .read(&mut body[offset..])
                    .map_err(|error| (408, error.to_string()))?;
                if read == 0 {
                    return Err((400, "Incomplete HTTP body".into()));
                }
                offset += read;
            }
            if path == "/v1/realtime" {
                serde_json::from_slice(&body)
                    .map(ApiOperation::Realtime)
                    .map_err(|error| (400, format!("Invalid realtime JSON: {error}")))
            } else {
                serde_json::from_slice(&body)
                    .map(ApiOperation::Play)
                    .map_err(|error| (400, format!("Invalid scenario JSON: {error}")))
            }
        }
        ("GET" | "POST", "/v1/graph" | "/v1/animation" | "/v1/animation/stop" | "/v1/realtime") => {
            Err((400, "Invalid method or unexpected request body".into()))
        }
        _ => Err((404, "Unknown endpoint".into())),
    }
}

fn set_header(slot: &mut Option<String>, value: String) -> Result<(), (u16, String)> {
    if slot.replace(value).is_some() {
        return Err((400, "Duplicate HTTP header".into()));
    }
    Ok(())
}

fn write_reply(stream: &mut Connection, reply: ApiReply) -> std::io::Result<()> {
    match stream {
        Connection::Plain(stream) => stream.write_deadline = Instant::now() + TIMEOUT,
        Connection::Tls(stream) => stream.sock.write_deadline = Instant::now() + TIMEOUT,
    }
    let body = reply.value.to_string();
    let reason = match reply.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        409 => "Conflict",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\r\n{body}",
        reply.status,
        body.len()
    )?;
    stream.flush()?;
    stream.flush()?;
    if let Connection::Tls(stream) = stream {
        stream.conn.send_close_notify();
        stream.flush()?;
    }
    let socket = match stream {
        Connection::Plain(socket) => socket,
        Connection::Tls(stream) => &mut stream.sock,
    };
    // Send FIN before dropping unread request data, which otherwise causes TCP resets.
    socket.stream.shutdown(Shutdown::Write)?;
    socket.deadline = Instant::now() + Duration::from_millis(20);
    let mut remaining = MAX_BODY + MAX_HEADERS;
    let mut buffer = [0; 4096];
    while remaining > 0 {
        let limit = buffer.len().min(remaining);
        match socket.read(&mut buffer[..limit]) {
            Ok(0) => break,
            Ok(read) => remaining -= read,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                ) =>
            {
                break;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
