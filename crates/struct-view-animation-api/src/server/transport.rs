use super::TIMEOUT;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Instant;

pub(super) struct DeadlineSocket {
    pub(super) stream: TcpStream,
    pub(super) deadline: Instant,
    pub(super) write_deadline: Instant,
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

pub(super) enum Connection {
    Plain(DeadlineSocket),
    Tls(Box<rustls::StreamOwned<rustls::ServerConnection, DeadlineSocket>>),
}

impl Connection {
    pub(super) fn plain(stream: TcpStream) -> Self {
        Self::Plain(DeadlineSocket {
            stream,
            deadline: Instant::now() + TIMEOUT,
            write_deadline: Instant::now() + TIMEOUT,
        })
    }

    pub(super) fn new(
        stream: TcpStream,
        config: Option<Arc<rustls::ServerConfig>>,
    ) -> std::io::Result<Self> {
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
