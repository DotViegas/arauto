//! The accept loop and the addresses of the two ports (`docs/14-protocolo-do-canal.md`,
//! section 1, and `docs/15-mvp.md`, sections 3 and 6).
//!
//! The loop is the project's own, over `hyper`: HTTP/1 only, 5 seconds for the headers of
//! the request (with the timer of `hyper-util`, also between two requests), `TCP_NODELAY`,
//! and the stop of the hub cancels every connection that is still in HTTP.
//!
//! | Port | Address | Answer |
//! |---|---|---|
//! | connectors | `GET /v1/channel` | the websocket, after the ceiling of connections without a session (503, 429) |
//! | connectors | `GET /v1/download/{file}` | a file of the fixed list, or 404 |
//! | internal | `GET /health` | `ok`, without any secret |
//! | either | anything else | 404 |

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::WS_MESSAGE_MAX;
use arauto_protocol::noise::Channel;
use axum::body::Body;
use axum::extract::FromRequestParts;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::http::{HeaderValue, Method, Request, Response, StatusCode, header};
use axum::response::IntoResponse;
use bytes::Bytes;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Instant;

use super::admission::{Origin, Permit, Refused};
use super::client_ip::client_ip;
use super::hello::converse;
use super::link::{self, LinkInfo, Start};
use super::settings::{READ_BUFFER_BYTES, duration};
use super::{Hub, Purposes, TaskGuard, handshake};

/// The path of the websocket.
pub const CHANNEL_PATH: &str = "/v1/channel";

/// The prefix of the downloads.
pub const DOWNLOAD_PREFIX: &str = "/v1/download/";

/// The path of the health check, on the internal port only.
pub const HEALTH_PATH: &str = "/health";

/// Largest buffer for the head of a request. Smaller would be refused by `hyper`.
const HEAD_BUFFER_BYTES: usize = 16 * 1024;

/// Longest name of a file to download.
const DOWNLOAD_NAME_MAX: usize = 128;

/// A name that cannot be on the list of downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "a file to download is named with up to 128 letters, digits, dots, hyphens and \
     underscores, and does not start with a dot"
)]
pub struct BadDownloadName;

/// The files of `GET /v1/download/{file}`: a fixed list, put together at the start.
///
/// The name in the address is only ever compared with the names of the list. It never
/// becomes a path: `..`, a slash or an encoded character simply match nothing. The list of
/// the first milestone is empty, so every download is 404.
#[derive(Debug, Clone, Default)]
pub struct Downloads {
    files: HashMap<String, Bytes>,
}

impl Downloads {
    /// No file.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Adds a file to the list.
    ///
    /// # Errors
    /// [`BadDownloadName`] for a name outside the narrow format.
    pub fn with_file(mut self, name: &str, content: Bytes) -> Result<Self, BadDownloadName> {
        let allowed = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_');
        if name.is_empty()
            || name.len() > DOWNLOAD_NAME_MAX
            || name.starts_with('.')
            || !name.bytes().all(allowed)
        {
            return Err(BadDownloadName);
        }
        self.files.insert(name.to_owned(), content);
        Ok(self)
    }

    fn get(&self, name: &str) -> Option<&Bytes> {
        self.files.get(name)
    }
}

/// Which of the two ports a connection came in by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Port {
    /// The port of the connectors, the only one that is published.
    Connectors,
    /// The internal port: health, and later the events of the engine.
    Internal,
}

/// Accepts connections until the hub stops.
pub(crate) async fn accept<P: Purposes>(
    listener: TcpListener,
    port: Port,
    hub: Arc<Hub>,
    purposes: Arc<P>,
) {
    let mut stopping = hub.stopping();
    loop {
        let accepted = tokio::select! {
            biased;
            _ = stopping.wait_for(|stopping| *stopping) => return,
            accepted = listener.accept() => accepted,
        };
        match accepted {
            Ok((stream, peer)) => {
                let guard = hub.tasks().guard();
                tokio::spawn(serve_http(
                    stream,
                    peer,
                    port,
                    Arc::clone(&hub),
                    Arc::clone(&purposes),
                    guard,
                ));
            }
            // Out of descriptors, or a connection that died before it was accepted: wait a
            // little instead of spinning on the same error.
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }
}

async fn serve_http<P: Purposes>(
    stream: TcpStream,
    peer: SocketAddr,
    port: Port,
    hub: Arc<Hub>,
    purposes: Arc<P>,
    _guard: TaskGuard,
) {
    // A frame of the channel never waits for the next one.
    let _ = stream.set_nodelay(true);
    let mut stopping = hub.stopping();
    let service = {
        let hub = Arc::clone(&hub);
        service_fn(move |request: Request<Incoming>| {
            let hub = Arc::clone(&hub);
            let purposes = Arc::clone(&purposes);
            async move { Ok::<_, Infallible>(route(request, peer, port, hub, purposes).await) }
        })
    };
    let mut builder = http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        // The deadline also holds between two requests of one connection: an idle one is cut.
        .header_read_timeout(duration(hub.settings().deadlines.http_headers_ms))
        .max_buf_size(HEAD_BUFFER_BYTES);
    let connection = builder
        .serve_connection(TokioIo::new(stream), service)
        .with_upgrades();
    tokio::select! {
        biased;
        _ = stopping.wait_for(|stopping| *stopping) => {}
        _ = connection => {}
    }
}

fn status(code: StatusCode) -> Response<Body> {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = code;
    response
}

async fn route<P: Purposes>(
    request: Request<Incoming>,
    peer: SocketAddr,
    port: Port,
    hub: Arc<Hub>,
    purposes: Arc<P>,
) -> Response<Body> {
    if request.method() != Method::GET {
        return status(StatusCode::NOT_FOUND);
    }
    let path = request.uri().path().to_owned();
    match port {
        Port::Connectors if path == CHANNEL_PATH => channel(request, peer, hub, purposes).await,
        Port::Connectors => match path.strip_prefix(DOWNLOAD_PREFIX) {
            Some(name) => download(&hub, name),
            None => status(StatusCode::NOT_FOUND),
        },
        Port::Internal if path == HEALTH_PATH => {
            let mut response = Response::new(Body::from("ok\n"));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            );
            response
        }
        Port::Internal => status(StatusCode::NOT_FOUND),
    }
}

fn download(hub: &Hub, name: &str) -> Response<Body> {
    match hub.downloads().get(name) {
        Some(content) => {
            let mut response = Response::new(Body::from(content.clone()));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            response
        }
        None => status(StatusCode::NOT_FOUND),
    }
}

/// `GET /v1/channel`: the ceiling, then the websocket.
async fn channel<P: Purposes>(
    request: Request<Incoming>,
    peer: SocketAddr,
    hub: Arc<Hub>,
    purposes: Arc<P>,
) -> Response<Body> {
    if hub.is_stopping() {
        return status(StatusCode::SERVICE_UNAVAILABLE);
    }
    let Ok(address) = client_ip(hub.client_ip_source(), peer, request.headers()) else {
        return status(StatusCode::BAD_REQUEST);
    };
    // Before the websocket opens: above the ceiling nothing of the channel is spent.
    let permit = match hub.admission().admit(Origin::of(address)) {
        Ok(permit) => permit,
        Err(Refused::HubFull) => return status(StatusCode::SERVICE_UNAVAILABLE),
        Err(Refused::OriginFull) => return status(StatusCode::TOO_MANY_REQUESTS),
    };
    let (mut parts, _body) = request.into_parts();
    let upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
        Ok(upgrade) => upgrade,
        Err(rejection) => return rejection.into_response(),
    };
    let guard = hub.tasks().guard();
    upgrade
        .read_buffer_size(READ_BUFFER_BYTES)
        // Each frame is written at once: the queues of the link are the buffer.
        .write_buffer_size(0)
        // The ceiling of a message is set in the library: a larger one is never put together.
        .max_message_size(WS_MESSAGE_MAX)
        .max_frame_size(WS_MESSAGE_MAX)
        .on_upgrade(move |socket| run(socket, address, permit, hub, purposes, guard))
}

/// One websocket, from its opening to its end.
async fn run<P: Purposes>(
    mut socket: WebSocket,
    address: IpAddr,
    permit: Permit,
    hub: Arc<Hub>,
    purposes: Arc<P>,
    _guard: TaskGuard,
) {
    let opened = Instant::now();
    let opened_at_ms = hub.now_ms();
    let deadline = opened + duration(hub.settings().deadlines.handshake_ms);
    let mut stopping = hub.stopping();
    let channel = tokio::select! {
        biased;
        _ = stopping.wait_for(|stopping| *stopping) => return,
        channel = handshake::respond(&mut socket, hub.keypair(), deadline) => match channel {
            Ok(channel) => channel,
            Err(handshake::Failed) => return,
        },
    };
    let Channel {
        handshake_hash,
        remote_static,
        sealer,
        opener,
    } = channel;
    let serial = hub.next_serial();
    let (link, tasks) = link::start(
        socket,
        Start {
            info: LinkInfo {
                serial,
                identity: remote_static,
                handshake_hash,
                address,
                opened_at_ms,
            },
            sealer,
            opener,
            permit,
            admission: Arc::clone(hub.admission()),
            budget: Arc::clone(hub.budget()),
            settings: hub.settings().clone(),
            opened,
        },
    );
    hub.register(serial, link.handle());
    // A stop that began while this handshake ended has already gone through the list.
    if hub.is_stopping() {
        link.close(ErrorCode::Ar205);
    }
    converse(Arc::clone(&hub), purposes, link).await;
    tasks.finish().await;
    hub.unregister(serial);
}
