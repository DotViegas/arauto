//! Plain HTTP/1.1 over TCP: the path goes exactly as written, the headers as slowly as a test
//! wants, and an upgrade can be left open without a websocket library on top.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Instant, timeout_at};

use crate::{Ended, KitError};

/// Largest head of a response this module reads.
const HEAD_MAX: usize = 16 * 1024;

/// Largest body of a response this module reads.
const BODY_MAX: usize = 1024 * 1024;

/// A response, as it came.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// The body, up to 1 MiB.
    pub body: Vec<u8>,
}

/// A request to open the websocket, and the TCP connection it went on.
#[derive(Debug)]
pub struct Upgrade {
    /// The status code: 101 when the hub opened the websocket.
    pub status: u16,
    /// The connection, still open. Dropped, it closes.
    pub stream: TcpStream,
}

fn request(path: &str, headers: &[(&str, &str)], extra: &str) -> String {
    let mut text = format!("GET {path} HTTP/1.1\r\nHost: arauto.test\r\n");
    for (name, value) in headers {
        text.push_str(name);
        text.push_str(": ");
        text.push_str(value);
        text.push_str("\r\n");
    }
    text.push_str(extra);
    text.push_str("\r\n");
    text
}

/// Reads the head of a response and returns its status and what came after the head.
async fn read_head(stream: &mut TcpStream, deadline: Instant) -> Result<(u16, Vec<u8>), KitError> {
    let mut head = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        if let Some(end) = head.windows(4).position(|window| window == b"\r\n\r\n") {
            let rest = head.split_off(end.saturating_add(4));
            let text = String::from_utf8_lossy(&head);
            let status = text
                .split(' ')
                .nth(1)
                .and_then(|code| code.parse().ok())
                .ok_or_else(|| KitError::Unexpected("a response without a status".into()))?;
            return Ok((status, rest));
        }
        if head.len() > HEAD_MAX {
            return Err(KitError::Unexpected("a head of response too long".into()));
        }
        let read = timeout_at(deadline, stream.read(&mut buffer))
            .await
            .map_err(|_| KitError::TimedOut)??;
        if read == 0 {
            return Err(KitError::Unexpected(
                "the connection closed before a response".into(),
            ));
        }
        head.extend_from_slice(buffer.get(..read).unwrap_or_default());
    }
}

/// `GET path`, with the path exactly as written.
///
/// # Errors
/// The network; [`KitError::TimedOut`].
pub async fn get(
    addr: SocketAddr,
    path: &str,
    headers: &[(&str, &str)],
    within: Duration,
) -> Result<Response, KitError> {
    let deadline = Instant::now() + within;
    let mut stream = TcpStream::connect(addr).await?;
    stream
        .write_all(request(path, headers, "Connection: close\r\n").as_bytes())
        .await?;
    let (status, mut body) = read_head(&mut stream, deadline).await?;
    let mut buffer = [0u8; 4096];
    while body.len() <= BODY_MAX {
        let read = timeout_at(deadline, stream.read(&mut buffer))
            .await
            .map_err(|_| KitError::TimedOut)??;
        if read == 0 {
            break;
        }
        body.extend_from_slice(buffer.get(..read).unwrap_or_default());
    }
    Ok(Response { status, body })
}

/// Asks for the websocket of the channel and leaves the connection as the hub left it: open
/// after 101, waiting for a handshake that never comes.
///
/// # Errors
/// The network; [`KitError::TimedOut`].
pub async fn upgrade(
    addr: SocketAddr,
    headers: &[(&str, &str)],
    within: Duration,
) -> Result<Upgrade, KitError> {
    let deadline = Instant::now() + within;
    let mut stream = TcpStream::connect(addr).await?;
    let extra = "Connection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
                 Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n";
    stream
        .write_all(request(crate::connector::CHANNEL_PATH, headers, extra).as_bytes())
        .await?;
    let (status, _) = read_head(&mut stream, deadline).await?;
    Ok(Upgrade { status, stream })
}

/// Writes the head of a request one header at a time, every `every`, and never ends it.
/// Returns how long the hub took to close the connection.
///
/// # Errors
/// [`KitError::TimedOut`] when the hub keeps the connection past `within`.
pub async fn slow_headers(
    addr: SocketAddr,
    every: Duration,
    within: Duration,
) -> Result<Ended, KitError> {
    let started = Instant::now();
    let deadline = started + within;
    let mut stream = TcpStream::connect(addr).await?;
    stream
        .write_all(b"GET /v1/channel HTTP/1.1\r\nHost: arauto.test\r\n")
        .await?;
    let mut buffer = [0u8; 1024];
    let mut header = 0u64;
    loop {
        let next = Instant::now() + every;
        match timeout_at(next.min(deadline), stream.read(&mut buffer)).await {
            // Closed, or reset: either way the hub let go.
            Ok(Ok(0) | Err(_)) => {
                return Ok(Ended {
                    closing: None,
                    after: started.elapsed(),
                });
            }
            // An answer to a request that never ended: wait for the close.
            Ok(Ok(_)) => {}
            Err(_) if Instant::now() >= deadline => return Err(KitError::TimedOut),
            Err(_) => {
                header = header.saturating_add(1);
                let line = format!("X-Slow-{header}: {header}\r\n");
                if stream.write_all(line.as_bytes()).await.is_err() {
                    return Ok(Ended {
                        closing: None,
                        after: started.elapsed(),
                    });
                }
            }
        }
    }
}

/// Waits for the hub to close a TCP connection, dropping whatever it sends.
///
/// # Errors
/// [`KitError::TimedOut`] when it is still open after `within`.
pub async fn closed(stream: &mut TcpStream, within: Duration) -> Result<Duration, KitError> {
    let started = Instant::now();
    let deadline = started + within;
    let mut buffer = [0u8; 4096];
    loop {
        match timeout_at(deadline, stream.read(&mut buffer)).await {
            Ok(Ok(0) | Err(_)) => return Ok(started.elapsed()),
            Ok(Ok(_)) => {}
            Err(_) => return Err(KitError::TimedOut),
        }
    }
}
