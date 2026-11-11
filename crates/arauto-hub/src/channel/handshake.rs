//! The Noise handshake over the websocket (`docs/14-protocolo-do-canal.md`, section 2).
//!
//! Three binary messages of exact sizes (48, 48, 64), and 10 seconds, counted from the
//! opening of the websocket, for the three of them. Anything else (a text message, another
//! size, a key that is nobody's, the deadline) closes the connection without an answer: there
//! is no cipher yet to carry a `closing`.

use arauto_protocol::noise::{Channel, HubHandshake, StaticKeypair};
use axum::extract::ws::{self, WebSocket};
use bytes::Bytes;
use futures_util::StreamExt;
use tokio::time::{Instant, timeout_at};

/// The handshake did not end. Nothing is said to the other side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Failed;

/// Answers the handshake of a connector before `deadline`.
pub(crate) async fn respond(
    socket: &mut WebSocket,
    keypair: &StaticKeypair,
    deadline: Instant,
) -> Result<Channel, Failed> {
    let work = async {
        let first = next_binary(socket).await?;
        let (handshake, second) = HubHandshake::respond(keypair, &first).map_err(|_| Failed)?;
        socket
            .send(ws::Message::Binary(Bytes::copy_from_slice(&second)))
            .await
            .map_err(|_| Failed)?;
        let third = next_binary(socket).await?;
        handshake.finish(&third).map_err(|_| Failed)
    };
    timeout_at(deadline, work).await.map_err(|_| Failed)?
}

/// The next binary message. The library answers websocket pings by itself; anything else
/// ends the handshake.
async fn next_binary(socket: &mut WebSocket) -> Result<Bytes, Failed> {
    loop {
        match socket.next().await {
            Some(Ok(ws::Message::Binary(bytes))) => return Ok(bytes),
            Some(Ok(ws::Message::Ping(_) | ws::Message::Pong(_))) => {}
            Some(Ok(ws::Message::Text(_) | ws::Message::Close(_)) | Err(_)) | None => {
                return Err(Failed);
            }
        }
    }
}
