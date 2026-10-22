use super::transport::Connection;
use super::{ApiOperation, ApiReply, ApiRequest, ApiTelemetry, MAX_BODY, MAX_HEADERS, TIMEOUT};
use serde_json::json;
use std::io::{BufReader, Read, Write};
use std::net::Shutdown;
use std::sync::{
    RwLock,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
};
use std::time::{Duration, Instant};

pub(super) fn serve(
    stream: &mut Connection,
    token: &RwLock<String>,
    hosts: &[String],
    sender: &SyncSender<ApiRequest>,
    wake: &(dyn Fn() + Send + Sync),
    shutdown: &AtomicBool,
    telemetry: &ApiTelemetry,
) -> std::io::Result<()> {
    let started = Instant::now();
    let parsed = parse_request(stream, token, hosts);
    let operation = match parsed {
        Ok(operation) => operation,
        Err((status, error)) => {
            let result = write_reply(
                stream,
                ApiReply {
                    status,
                    value: json!({"error":error}),
                },
            );
            telemetry.record("Invalid HTTP request", "", status, started.elapsed());
            return result;
        }
    };
    let endpoint = operation.endpoint();
    let summary = operation.summary();
    let (reply, received) = mpsc::sync_channel(1);
    let deadline = Instant::now() + TIMEOUT;
    telemetry.queued_requests.fetch_add(1, Ordering::Relaxed);
    if sender
        .try_send(ApiRequest {
            operation,
            reply,
            deadline,
        })
        .is_err()
    {
        telemetry.queued_requests.fetch_sub(1, Ordering::Relaxed);
        let result = write_reply(
            stream,
            ApiReply {
                status: 503,
                value: json!({"error":"API queue is full or closed"}),
            },
        );
        telemetry.record(endpoint, &summary, 503, started.elapsed());
        return result;
    }
    wake();
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
                    value: json!({"error":"Application request channel closed"}),
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
    let status = response.status;
    let result = write_reply(stream, response);
    telemetry.record(endpoint, &summary, status, started.elapsed());
    result
}

pub(super) type ParseResult = Result<ApiOperation, (u16, String)>;

pub(super) fn parse_request(
    stream: &mut Connection,
    token: &RwLock<String>,
    hosts: &[String],
) -> ParseResult {
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
        ("GET", "/v1/capabilities") if length == 0 => Ok(ApiOperation::Capabilities),
        ("GET", "/v1/graph") if length == 0 => Ok(ApiOperation::Graph),
        ("GET", "/v1/animation") if length == 0 => Ok(ApiOperation::Status),
        ("GET", "/v1/realtime") if length == 0 => Ok(ApiOperation::RealtimeStatus),
        ("GET", "/v1/jobs") if length == 0 => Ok(ApiOperation::JobsStatus),
        ("POST", "/v1/animation/stop") if length == 0 => Ok(ApiOperation::Stop),
        ("POST", path @ ("/v1/animation" | "/v1/realtime" | "/v1/jobs")) => {
            if !content_type.as_deref().is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
            }) {
                return Err((415, "Content-Type must be application/json".into()));
            }
            if length == 0 {
                return Err((400, "A JSON request body is required".into()));
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
            let value = serde_json::from_slice(&body)
                .map_err(|error| (400, format!("Invalid JSON: {error}")))?;
            Ok(match path {
                "/v1/realtime" => ApiOperation::Realtime(value),
                "/v1/jobs" => ApiOperation::Jobs(value),
                _ => ApiOperation::Play(value),
            })
        }
        (
            "GET" | "POST",
            "/v1/graph" | "/v1/animation" | "/v1/animation/stop" | "/v1/realtime" | "/v1/jobs"
            | "/v1/capabilities",
        ) => Err((400, "Invalid method or unexpected request body".into())),
        _ => Err((404, "Unknown endpoint".into())),
    }
}

fn set_header(slot: &mut Option<String>, value: String) -> Result<(), (u16, String)> {
    if slot.replace(value).is_some() {
        return Err((400, "Duplicate HTTP header".into()));
    }
    Ok(())
}

pub(super) fn write_reply(stream: &mut Connection, reply: ApiReply) -> std::io::Result<()> {
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
