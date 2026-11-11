//! The enrollment: `purpose: enroll` (`docs/14-protocolo-do-canal.md`, section 5.2;
//! `docs/04-seguranca-conexao.md`, C3 and C4).
//!
//! A short connection: one `enroll`, one answer, and the connection closes.
//!
//! ```text
//!  connector  enroll   invite, declared (hostname, os, hermes_version)
//!  hub        enrolled nonce, confirm_within_s, enrollment       or   closing AR-1xx / AR-30x
//! ```
//!
//! - `enroll` only comes after `hello_ok`, once. Before it the transport closes (the first
//!   message is `hello`); a second one is never read, since the connection closes after the
//!   answer.
//! - The declared texts are checked when the message is read: one line, at most 128
//!   characters, no control or direction character. Otherwise the message is invalid and the
//!   connection closes with AR-204.
//! - An invite whose digest belongs to no organization is refused with AR-101 and counted in
//!   memory, by origin and minute ([`RefusedEnrollments`](crate::repo::enroll::RefusedEnrollments)):
//!   the sweep writes the minutes that ended.
//! - The nonce is drawn so that a code can be taken from it without bias, and the code is
//!   computed and stored by the repository. The hub never sends it.

use std::sync::Arc;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{Hello, Message};

use super::Hub;
use super::link::{Link, RecvError};
use crate::db;
use crate::repo::enroll::{self, EnrollOutcome, Enrollment};

/// `purpose: enroll`, from the first message after `hello_ok` to the answer.
pub(crate) async fn run(hub: Arc<Hub>, mut link: Link, hello: Hello) {
    let enrolled = match link
        .recv_within(hub.settings().deadlines.connector_reply_ms)
        .await
    {
        Ok(Message::Enroll(enrolled)) => enrolled,
        Ok(_) | Err(RecvError::TimedOut) => return link.close(ErrorCode::Ar204),
        Err(RecvError::Closed) => return,
    };
    let info = link.info().clone();
    // Only the digest of the invite is kept; the invite itself is wiped when dropped.
    let digest = enrolled.invite.digest();
    let declared = enrolled.declared;
    drop(enrolled.invite);
    let now = hub.now_ms();
    let organization = match db::organization_of_invite(hub.pool(), &digest).await {
        Ok(Some(organization)) => organization,
        Ok(None) => {
            // No organization ever issued it: AR-101, and the refusal is only counted.
            hub.refusals()
                .note(None, Some(info.address), ErrorCode::Ar101, now);
            return link.close(ErrorCode::Ar101);
        }
        Err(error) => {
            tracing::error!(%error, "the organization of an invite could not be read");
            return link.close(ErrorCode::Ar205);
        }
    };
    let Ok(nonce) = enroll::draw_nonce(&info.handshake_hash) else {
        tracing::error!("the generator of the system failed");
        return link.close(ErrorCode::Ar205);
    };
    let request = Enrollment {
        identity: &info.identity,
        declared: &declared,
        connector_version: &hello.connector,
        origin: Some(info.address),
        nonce: &nonce,
        handshake_hash: &info.handshake_hash,
    };
    let outcome = enroll::enroll(
        hub.pool(),
        hub.mark(),
        organization,
        hub.refusals(),
        &digest,
        &request,
        now,
    )
    .await;
    match outcome {
        Ok(EnrollOutcome::Enrolled(accepted)) => {
            // The code is computed on each side; the connector shows it and closes.
            let _ = link.send(&Message::Enrolled(accepted.reply)).await;
        }
        Ok(EnrollOutcome::Refused(refusal)) => {
            // The request this enrollment closed may have a waiting connection.
            if let Some(device) = refusal.closed {
                for entry in hub.registry().of(device) {
                    entry.poke();
                }
            }
            link.close(refusal.code);
        }
        Err(error) => {
            link.close(ErrorCode::Ar205);
            hub.security_write_failed(organization, None, error).await;
        }
    }
}
