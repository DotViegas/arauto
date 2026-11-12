//! The tools of the conversation between Hermes, answered by the hub in a session of the
//! channel (`docs/13-conversa-entre-hermes.md`; `docs/09-contrato-mcp.md`).
//!
//! Every call decides against the list of peers of now, in the database: a link the admin
//! took away holds at the next message and at the next read, whatever list of tools the
//! session was given. The session read the device again and checked the name and every
//! argument before a call gets here.
//!
//! | Tool | Refusals |
//! |---|---|
//! | every one | AR-701 when no link of the Hermes releases the tool: sending needs a link from it; the other three, a link in either direction. |
//! | `hermes_list_peers` | none besides. |
//! | `hermes_send_message` | AR-707 (no peer of that name it may send to: the same for a name that does not exist), AR-708 (the thread), AR-706 with `retry_after_s` (a ceiling). |
//! | `hermes_list_threads` | AR-708 (`before` names no thread it may see). |
//! | `hermes_read_thread` | AR-708 (the thread does not exist or is of others), AR-711 (`before` or `after` names no message of it). |
//!
//! The text of a message is whatever another agent wrote: it goes back as data, in the field
//! `text`, and nowhere else. Nothing of it is logged or audited.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::HOUR_MS;
use arauto_protocol::message::{CallOutcome, Payload};
use arauto_protocol::names::HermesName;
use arauto_protocol::tools::{Grants, ListThreads, ReadThread, SendMessage};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::channel::Hub;
use crate::db::ResolvedOrg;
use crate::repo::peer::{self, NewMessage, Read, ReadRequest, Sent};
use crate::repo::{self, RepoError};

/// One call of the conversation between Hermes.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PeerCall<'a> {
    /// `hermes_list_peers`
    ListPeers,
    /// `hermes_send_message`
    Send(&'a SendMessage),
    /// `hermes_list_threads`
    ListThreads(ListThreads),
    /// `hermes_read_thread`
    ReadThread(ReadThread),
}

fn refused(code: ErrorCode) -> CallOutcome {
    CallOutcome::Refused {
        code,
        retry_after: None,
    }
}

/// A failure of the hub itself, logged without any value of the call: AR-205.
fn failed(error: &RepoError, what: &'static str) -> CallOutcome {
    tracing::error!(%error, "{what}");
    refused(ErrorCode::Ar205)
}

fn content(value: Value) -> CallOutcome {
    match value {
        Value::Object(content) => CallOutcome::Ok(Payload::new(content)),
        _ => refused(ErrorCode::Ar205),
    }
}

/// The grants of the links of a Hermes now: what its list of tools follows.
pub(crate) async fn grants_now(
    hub: &Hub,
    organization: ResolvedOrg,
    hermes: Uuid,
    grants: Grants,
) -> Result<Grants, RepoError> {
    let links = repo::hermes::peer_links(hub.pool(), organization, hermes).await?;
    Ok(Grants {
        peer_any: links.outbound || links.inbound,
        peer_outbound: links.outbound,
        ..grants
    })
}

/// What answers a call of the conversation between Hermes. `name` is the Hermes that calls.
pub(crate) async fn answer(
    hub: &Hub,
    organization: ResolvedOrg,
    hermes: Uuid,
    name: &HermesName,
    call: PeerCall<'_>,
) -> CallOutcome {
    // A tool no link releases is not released (AR-701), whatever list the connector has.
    let links = match repo::hermes::peer_links(hub.pool(), organization, hermes).await {
        Ok(links) => links,
        Err(error) => return failed(&error, "the links of a Hermes could not be read"),
    };
    let released = match call {
        PeerCall::Send(_) => links.outbound,
        PeerCall::ListPeers | PeerCall::ListThreads(_) | PeerCall::ReadThread(_) => {
            links.outbound || links.inbound
        }
    };
    if !released {
        return refused(ErrorCode::Ar701);
    }
    match call {
        PeerCall::ListPeers => list_peers(hub, organization, hermes).await,
        PeerCall::Send(message) => send(hub, organization, hermes, message).await,
        PeerCall::ListThreads(list) => list_threads(hub, organization, hermes, list).await,
        PeerCall::ReadThread(read) => read_thread(hub, organization, hermes, name, read).await,
    }
}

async fn list_peers(hub: &Hub, organization: ResolvedOrg, hermes: Uuid) -> CallOutcome {
    match peer::peers_of(hub.pool(), organization, hermes).await {
        Ok(peers) => {
            let peers: Vec<Value> = peers
                .iter()
                .map(|peer| {
                    json!({
                        "name": peer.name.as_str(),
                        "can_send": peer.can_send,
                        "can_receive": peer.can_receive,
                    })
                })
                .collect();
            content(json!({ "peers": peers }))
        }
        Err(error) => failed(&error, "the peers of a Hermes could not be read"),
    }
}

async fn send(
    hub: &Hub,
    organization: ResolvedOrg,
    hermes: Uuid,
    message: &SendMessage,
) -> CallOutcome {
    let key = match hub.organization_key(organization).await {
        Ok(key) => key,
        Err(error) => return failed(&error, "the key of an organization could not be opened"),
    };
    let new = NewMessage {
        from: hermes,
        to: message.to.as_ref(),
        thread: message.thread,
        text: &message.text,
    };
    let now = hub.now_ms();
    match peer::send(hub.pool(), organization, &key, new, now).await {
        Ok(Sent::Done {
            thread,
            message,
            to,
            noticed,
        }) => {
            if noticed {
                poke(hub, to);
            }
            content(json!({
                "thread": thread.as_hyphenated().to_string(),
                "message": message.as_hyphenated().to_string(),
            }))
        }
        Ok(Sent::NoPeer) => refused(ErrorCode::Ar707),
        Ok(Sent::NoThread) => refused(ErrorCode::Ar708),
        Ok(Sent::Ceiling {
            which,
            retry_after,
            to,
        }) => {
            if first_of_the_hour(hermes, to, now)
                && let Err(error) =
                    peer::record_ceiling(hub.pool(), organization, hermes, to, which, now).await
            {
                tracing::error!(%error, "a message ceiling could not be audited");
            }
            CallOutcome::Refused {
                code: ErrorCode::Ar706,
                retry_after,
            }
        }
        Err(error) => failed(&error, "a message between Hermes could not be kept"),
    }
}

async fn list_threads(
    hub: &Hub,
    organization: ResolvedOrg,
    hermes: Uuid,
    list: ListThreads,
) -> CallOutcome {
    match peer::threads(hub.pool(), organization, hermes, list.limit, list.before).await {
        Ok(Some(page)) => {
            let threads: Vec<Value> = page
                .threads
                .iter()
                .map(|thread| {
                    json!({
                        "thread": thread.id.as_hyphenated().to_string(),
                        "peer": thread.peer.as_str(),
                        "last_message_at": thread.last_message_at,
                        "unread": thread.unread,
                    })
                })
                .collect();
            content(json!({ "threads": threads, "more": page.more }))
        }
        Ok(None) => refused(ErrorCode::Ar708),
        Err(error) => failed(&error, "the threads of a Hermes could not be read"),
    }
}

async fn read_thread(
    hub: &Hub,
    organization: ResolvedOrg,
    hermes: Uuid,
    name: &HermesName,
    read: ReadThread,
) -> CallOutcome {
    let key = match hub.organization_key(organization).await {
        Ok(key) => key,
        Err(error) => return failed(&error, "the key of an organization could not be opened"),
    };
    let request = ReadRequest {
        hermes,
        thread: read.thread,
        limit: read.limit,
        before: read.before,
        after: read.after,
    };
    match peer::read(hub.pool(), organization, &key, request, hub.now_ms()).await {
        Ok(Read::Messages {
            peer,
            messages,
            more,
        }) => {
            let messages: Vec<Value> = messages
                .iter()
                .map(|message| {
                    let from = if message.mine { name } else { &peer };
                    json!({
                        "id": message.id.as_hyphenated().to_string(),
                        "from": from.as_str(),
                        "mine": message.mine,
                        "sent_at": message.sent_at,
                        "text": message.text.as_str(),
                    })
                })
                .collect();
            content(json!({
                "thread": read.thread.as_hyphenated().to_string(),
                "peer": peer.as_str(),
                "messages": messages,
                "more": more,
            }))
        }
        Ok(Read::NoThread) => refused(ErrorCode::Ar708),
        Ok(Read::NoCursor) => refused(ErrorCode::Ar711),
        Err(error) => failed(&error, "a thread could not be read"),
    }
}

/// The sessions of `hermes` look at their queue of notices now.
pub(crate) fn poke(hub: &Hub, hermes: Uuid) {
    for entry in hub.registry().all() {
        if entry.place().is_some_and(|place| place.hermes == hermes) {
            entry.poke();
        }
    }
}

/// How many links are remembered before the idle ones are dropped.
const LINKS_BEFORE_PRUNE: usize = 4_096;

/// Whether this is the first ceiling of the link from `from` to `to` in this hour: the audit
/// gets one event per link and per hour, never one per refused message.
fn first_of_the_hour(from: Uuid, to: Uuid, now: i64) -> bool {
    static SEEN: OnceLock<Mutex<HashMap<(Uuid, Uuid), i64>>> = OnceLock::new();
    // Plain numbers: a panic elsewhere leaves them usable.
    let mut seen = SEEN
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if seen.len() > LINKS_BEFORE_PRUNE {
        seen.retain(|_, at| now.saturating_sub(*at) < HOUR_MS);
    }
    match seen.get(&(from, to)) {
        Some(at) if now.saturating_sub(*at) < HOUR_MS && now >= *at => false,
        _ => {
            seen.insert((from, to), now);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_is_audited_once_an_hour() {
        let (from, to) = (Uuid::new_v4(), Uuid::new_v4());
        assert!(first_of_the_hour(from, to, 1_000));
        assert!(!first_of_the_hour(from, to, 2_000));
        assert!(!first_of_the_hour(from, to, 1_000 + HOUR_MS - 1));
        assert!(first_of_the_hour(from, to, 1_000 + HOUR_MS));
        // The other direction is another link.
        assert!(first_of_the_hour(to, from, 2_000));
    }
}
