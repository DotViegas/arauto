//! From a message that arrived on a number to the notices of the Hermes it wakes
//! (`docs/08-permissoes.md`, section 4; `docs/09-contrato-mcp.md`, section 5;
//! `docs/10-maquinas-de-estado.md`, section 5).
//!
//! A message wakes a Hermes when, at that moment: the number is connected; the Hermes may be
//! woken and has an active device; and the matrix gives it the verb "be woken" in that chat
//! (the line of the chat when there is one, which replaces the verbs of the number whole;
//! otherwise the verbs of the number, with the scope "all", and only in a chat with one
//! person: a group wakes only by a line of its own). A message the number itself sent wakes
//! nobody. An edit, a deletion and a reaction are recorded and wake nobody.
//!
//! **The wait.** Whoever writes usually sends a few messages in a row. The notice of a chat
//! leaves only after the wait of the access (`wake_wait_ms`) passed without a new message
//! in that chat, and never more than [`WAKE_WAIT_MAX_MS`] after the first one: the Hermes is
//! woken once, with the whole of what was written.
//!
//! The notice is a pointer: the number, the chat and how many. One pending notice per Hermes
//! and chat: what comes next adds to its count. It is judged again when it leaves
//! ([`super::peer::lease`]).
//!
//! The event is handled in the transaction that recorded it: it is never left open.

use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

use super::peer::{NOTICE_LIFE_MS, NOTICES_PER_HERMES_MAX};

/// The longest a notice waits for the person to stop writing, from its first message.
pub const WAKE_WAIT_MAX_MS: i64 = 60_000;

/// The wait an access has when the admin set none.
pub const WAKE_WAIT_DEFAULT_MS: i32 = 6_000;
use super::{RepoError, chat, engine_event};
use crate::db::ResolvedOrg;
use crate::engine::Author;
use crate::engine::waha::parse_message;
use crate::vault::OrganizationKey;

/// Handles the event `event`, a message of `number` whose payload is `payload`: creates or
/// grows the notices of the Hermes it wakes, and marks the event as handled. Returns the
/// Hermes whose sessions have something to send.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses; [`RepoError::Vault`] when the chat
/// cannot be sealed.
pub async fn on_message(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    event: Uuid,
    payload: &Value,
    now: i64,
) -> Result<Vec<Uuid>, RepoError> {
    let woken = match parse_message(payload, None) {
        // The number itself wrote it, or it is not a message the hub can read.
        Some(message) if message.author != Author::Me => {
            let chat =
                chat::resolve(connection, organization, key, number, &message.chat, now).await?;
            notify(connection, organization, number, chat, now).await?
        }
        _ => Vec::new(),
    };
    engine_event::handled(connection, organization, event, now).await?;
    Ok(woken)
}

/// The notices of one new message in `chat` of `number`.
async fn notify(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    number: Uuid,
    chat: Uuid,
    now: i64,
) -> Result<Vec<Uuid>, RepoError> {
    // The messages of a number are handled one at a time, under the lock of the number: two
    // at once would both create the pending notice of the same chat.
    let connected: Option<bool> = sqlx::query_scalar(
        "SELECT state = 'connected' FROM number \
         WHERE organization_id = $1 AND id = $2 FOR NO KEY UPDATE",
    )
    .bind(organization.id())
    .bind(number)
    .fetch_optional(&mut *connection)
    .await?;
    if connected != Some(true) {
        return Ok(Vec::new());
    }
    let woken: Vec<(Uuid, i32)> = sqlx::query_as(
        "SELECT a.hermes_id, a.wake_wait_ms FROM access a \
         JOIN hermes h ON h.organization_id = a.organization_id AND h.id = a.hermes_id \
         JOIN chat ch ON ch.organization_id = a.organization_id AND ch.id = $3 \
         LEFT JOIN access_chat c ON c.access_id = a.id AND c.chat_id = $3 \
         WHERE a.organization_id = $1 AND a.number_id = $2 \
           AND h.deleted_at IS NULL AND h.can_be_woken \
           AND COALESCE(c.can_wake, a.scope = 'all' AND a.can_wake AND ch.kind = 'contact') \
           AND EXISTS (SELECT 1 FROM device d \
                       WHERE d.organization_id = $1 AND d.hermes_id = a.hermes_id \
                         AND d.state = 'active') \
         ORDER BY a.hermes_id",
    )
    .bind(organization.id())
    .bind(number)
    .bind(chat)
    .fetch_all(&mut *connection)
    .await?;
    let hermes: Vec<Uuid> = woken.iter().map(|(hermes, _)| *hermes).collect();
    for (hermes, wait) in &woken {
        let leaves_at = now.saturating_add(i64::from(*wait));
        // One more message: the notice waits again, up to a minute from its first one.
        let grown = sqlx::query(
            "UPDATE notice SET count = LEAST(count + 1, 9999), \
                    not_before = LEAST(GREATEST(not_before, $4), created_at + $5) \
             WHERE organization_id = $1 AND kind = 'whatsapp' AND hermes_id = $2 \
               AND chat_id = $3 AND state = 'pending'",
        )
        .bind(organization.id())
        .bind(hermes)
        .bind(chat)
        .bind(leaves_at)
        .bind(WAKE_WAIT_MAX_MS)
        .execute(&mut *connection)
        .await?
        .rows_affected();
        if grown > 0 {
            continue;
        }
        // The queue of a Hermes holds 1,000: past it, the oldest expires.
        sqlx::query(
            "UPDATE notice SET state = 'expired', resolved_at = $3, leased_until = NULL \
             WHERE id IN (SELECT id FROM notice \
                          WHERE organization_id = $1 AND hermes_id = $2 \
                            AND state IN ('pending', 'sent') \
                          ORDER BY created_at DESC, id OFFSET $4)",
        )
        .bind(organization.id())
        .bind(hermes)
        .bind(now)
        .bind(NOTICES_PER_HERMES_MAX - 1)
        .execute(&mut *connection)
        .await?;
        sqlx::query(
            "INSERT INTO notice (id, organization_id, hermes_id, kind, number_id, chat_id, \
                                 count, state, not_before, created_at, expires_at) \
             VALUES ($1, $2, $3, 'whatsapp', $4, $5, 1, 'pending', $8, $6, $7)",
        )
        .bind(Uuid::new_v4())
        .bind(organization.id())
        .bind(hermes)
        .bind(number)
        .bind(chat)
        .bind(now)
        .bind(now.saturating_add(NOTICE_LIFE_MS))
        .bind(leaves_at)
        .execute(&mut *connection)
        .await?;
    }
    Ok(hermes)
}
