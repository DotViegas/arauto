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
    /// The headers, in the order they came, with their names in lowercase.
    pub headers: Vec<(String, String)>,
    /// The body, up to 1 MiB.
    pub body: Vec<u8>,
}

impl Response {
    /// The value of a header that came once. `None` when it did not come, or came twice.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        let mut found = self.all(name);
        match (found.next(), found.next()) {
            (Some(value), None) => Some(value),
            _ => None,
        }
    }

    /// Every value of a header.
    pub fn all<'r>(&'r self, name: &str) -> impl Iterator<Item = &'r str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .filter(move |(found, _)| *found == name)
            .map(|(_, value)| value.as_str())
    }

    /// The body, as text.
    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
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
    head("GET", path, headers, extra)
}

/// The head of a request. `Host: arauto.test` goes unless the test names another.
fn head(method: &str, path: &str, headers: &[(&str, &str)], extra: &str) -> String {
    let mut text = format!("{method} {path} HTTP/1.1\r\n");
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("host"))
    {
        text.push_str("Host: arauto.test\r\n");
    }
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
    let (status, _, rest) = read_whole_head(stream, deadline).await?;
    Ok((status, rest))
}

/// The headers of a head, with their names in lowercase.
fn headers_of(head: &str) -> Vec<(String, String)> {
    head.split("\r\n")
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect()
}

/// Reads the head of a response: its status, its headers and what came after it.
async fn read_whole_head(
    stream: &mut TcpStream,
    deadline: Instant,
) -> Result<(u16, Vec<(String, String)>, Vec<u8>), KitError> {
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
            return Ok((status, headers_of(&text), rest));
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
    send("GET", addr, path, headers, &[], within).await
}

/// `POST path` with a body. `Content-Length` is added; the type of the body is the test's
/// to say.
///
/// # Errors
/// The network; [`KitError::TimedOut`].
pub async fn post(
    addr: SocketAddr,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    within: Duration,
) -> Result<Response, KitError> {
    send("POST", addr, path, headers, body, within).await
}

/// One request with any method, on a connection of its own.
///
/// # Errors
/// The network; [`KitError::TimedOut`].
pub async fn send(
    method: &str,
    addr: SocketAddr,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    within: Duration,
) -> Result<Response, KitError> {
    let deadline = Instant::now() + within;
    let mut stream = TcpStream::connect(addr).await?;
    let extra = if body.is_empty() && method == "GET" {
        "Connection: close\r\n".to_owned()
    } else {
        format!("Content-Length: {}\r\nConnection: close\r\n", body.len())
    };
    let mut bytes = head(method, path, headers, &extra).into_bytes();
    bytes.extend_from_slice(body);
    stream.write_all(&bytes).await?;
    let (status, headers, mut body) = read_whole_head(&mut stream, deadline).await?;
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
    Ok(Response {
        status,
        headers,
        body,
    })
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
