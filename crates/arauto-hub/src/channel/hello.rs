//! The first message, the version, and the purpose (`docs/14-protocolo-do-canal.md`,
//! sections 5.1 and 5.4).
//!
//! The connector speaks first, with `hello`, within 5 seconds of the handshake. The hub
//! answers with the highest version in common and hands the connection to the code of its
//! purpose: the enrollment, the channel, or the request to leave, which lives here because it
//! is short and complete.
//!
//! | Versions | `enroll` | `channel` | `leave` |
//! |---|---|---|---|
//! | in common | `hello_ok` | `hello_ok` | `hello_ok` |
//! | connector too old | `closing` AR-401 | `hello_ok` outdated, then the presentation | `hello_ok` outdated, then served |
//! | connector too new | `closing` AR-402 | `closing` AR-402 | `closing` AR-402 |

use std::net::IpAddr;
use std::sync::Arc;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{self, HelloOk, LeftStatus, Message, Negotiation, Purpose};
use arauto_protocol::noise::PublicKey;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::link::{Link, RecvError};
use super::{Hub, Purposes};
use crate::audit::AuditError;
use crate::db::{self, ResolvedOrg};
use crate::mark::Mark;
use crate::repo::{self, RepoError};
use crate::restore::Evidence;

/// From the first message to the code of the purpose.
pub(crate) async fn converse<P: Purposes>(hub: Arc<Hub>, purposes: Arc<P>, mut link: Link) {
    let hello = match link.recv_within(hub.settings().deadlines.hello_ms).await {
        Ok(Message::Hello(hello)) => hello,
        // The first message is `hello`, and it comes in time.
        Ok(_) | Err(RecvError::TimedOut) => return link.close(ErrorCode::Ar204),
        Err(RecvError::Closed) => return,
    };
    let answer = match (
        hub.settings().versions.negotiate(hello.proto),
        hello.purpose,
    ) {
        (Negotiation::ConnectorTooNew, _) => return link.close(ErrorCode::Ar402),
        (Negotiation::ConnectorTooOld, Purpose::Enroll) => return link.close(ErrorCode::Ar401),
        (Negotiation::ConnectorTooOld, Purpose::Channel | Purpose::Leave) => HelloOk::Outdated,
        (Negotiation::Agreed(version), _) => HelloOk::Agreed(version),
    };
    // From `hello_ok` on, each side sends `ping` every 30 seconds.
    link.start_heartbeat();
    if link.send(&Message::HelloOk(answer)).await.is_err() {
        return;
    }
    match hello.purpose {
        Purpose::Enroll => purposes.enroll(hub, link, hello).await,
        Purpose::Channel => purposes.channel(hub, link, hello, answer).await,
        Purpose::Leave => leave(&hub, link).await,
    }
}

/// `purpose: leave`: a short connection, one per identity. It presents no credential and is
/// never a double connection (C9). The three answers let the connector delete the folder.
async fn leave(hub: &Hub, mut link: Link) {
    match link
        .recv_within(hub.settings().deadlines.connector_reply_ms)
        .await
    {
        Ok(Message::Leave) => {}
        Ok(_) | Err(RecvError::TimedOut) => return link.close(ErrorCode::Ar204),
        Err(RecvError::Closed) => return,
    }
    let info = link.info().clone();
    let left = leave_by_identity(
        hub.pool(),
        hub.mark(),
        &info.identity,
        info.address,
        hub.now_ms(),
    )
    .await;
    match left {
        Ok(Left { status, unfollowed }) => {
            // The closing is committed; only the mark did not follow. A database ahead of the
            // mark accuses nothing, but a restoration that lost exactly this event would go
            // unseen: the admin is told, as on the open channel, and the alert is written
            // before the connector hears `left` and deletes its folder.
            if let Some((organization, device, error)) = unfollowed {
                hub.security_write_failed(organization, Some(device), error)
                    .await;
            }
            let _ = link.send(&Message::Left(message::Left { status })).await;
        }
        Err(LeaveFailed::Restored(evidence)) => {
            // Nothing was written: the database came back from a copy. The hub turns the
            // restored mode on and every connection reconnects into it.
            hub.enter_restored_mode(evidence).await;
            link.close(ErrorCode::Ar205);
        }
        Err(LeaveFailed::Unavailable) => link.close(ErrorCode::Ar205),
    }
}

/// What a request to leave did.
struct Left {
    status: LeftStatus,
    /// The closing is committed, but the audit mark did not follow it: of which device.
    unfollowed: Option<(ResolvedOrg, Uuid, RepoError)>,
}

impl From<LeftStatus> for Left {
    fn from(status: LeftStatus) -> Self {
        Self {
            status,
            unfollowed: None,
        }
    }
}

/// Why a request to leave got no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaveFailed {
    /// The database is behind the mark; nothing was written.
    Restored(Evidence),
    /// The database or the mark did not answer; nothing is known to be written.
    Unavailable,
}

/// Closes the device of `identity`, found by the entry function of the identity.
async fn leave_by_identity(
    pool: &PgPool,
    mark: &Mark,
    identity: &PublicKey,
    address: IpAddr,
    now: i64,
) -> Result<Left, LeaveFailed> {
    let unavailable = |_| LeaveFailed::Unavailable;
    let Some(organization) = db::organization_of_device(pool, identity)
        .await
        .map_err(unavailable)?
    else {
        return Ok(LeftStatus::Unknown.into());
    };
    let Some(device) = repo::device::by_identity(pool, organization, identity)
        .await
        .map_err(|_| LeaveFailed::Unavailable)?
    else {
        return Ok(LeftStatus::Unknown.into());
    };
    match repo::closing::leave(pool, mark, organization, device.id, Some(address), now).await {
        Ok(left) => Ok(left.status.into()),
        Err(RepoError::Restored(evidence)) => Err(LeaveFailed::Restored(evidence)),
        Err(error @ RepoError::Audit(AuditError::MarkNotAdvanced { .. })) => Ok(Left {
            status: LeftStatus::Closed,
            unfollowed: Some((organization, device.id, error)),
        }),
        Err(_) => Err(LeaveFailed::Unavailable),
    }
}
