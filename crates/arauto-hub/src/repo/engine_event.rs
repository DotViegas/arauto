//! The inbox of events of the engine (`docs/06-modelo-de-dados.md`, section 6).
//!
//! An event is written before the hub answers the engine, and leaves only after it is
//! handled. What makes an event one is kept as a keyed digest (a message is one event whether
//! it came from the engine or from the catch-up, and the same event delivered twice is one
//! row); its body is encrypted with the key of the organization.
//!
//! The statuses of a session are handled when they arrive ([`super::number::observe`]). The
//! messages, reactions, edits and deletions stay open for the queue of notices, which comes
//! with the second milestone; the purge of the handled ones, after a day, with the purge task.

use sqlx::postgres::{PgConnection, PgPool};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::RepoError;
use crate::db::ResolvedOrg;
use crate::vault::{OrganizationKey, WhatsappField};

/// The kind of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// The status of a session of the engine.
    SessionStatus,
    /// A message, received or sent.
    Message,
    /// A reaction to a message.
    Reaction,
    /// A message was edited.
    Edited,
    /// A message was deleted.
    Revoked,
}

impl EventKind {
    /// Every kind.
    pub const ALL: &'static [EventKind] = &[
        EventKind::SessionStatus,
        EventKind::Message,
        EventKind::Reaction,
        EventKind::Edited,
        EventKind::Revoked,
    ];

    /// The name of the event in the engine, which is also the text stored.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            EventKind::SessionStatus => "session.status",
            EventKind::Message => "message.any",
            EventKind::Reaction => "message.reaction",
            EventKind::Edited => "message.edited",
            EventKind::Revoked => "message.revoked",
        }
    }

    /// Reads the name of the event of the engine.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|kind| kind.as_str() == text)
    }
}

/// Where an event came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The engine sent it to the inbox.
    Webhook,
    /// The hub asked the engine for what arrived while it was away.
    CatchUp,
}

impl Source {
    const fn as_str(self) -> &'static str {
        match self {
            Source::Webhook => "webhook",
            Source::CatchUp => "catch_up",
        }
    }
}

/// An event to write.
pub struct NewEvent<'a> {
    /// The engine.
    pub engine: Uuid,
    /// The number of the session.
    pub number: Uuid,
    /// The kind.
    pub kind: EventKind,
    /// What makes it one: the identifier of the message, or of the event of the engine.
    pub key: &'a [u8],
    /// The body, as the engine gave it.
    pub body: &'a [u8],
    /// When, in milliseconds of the engine.
    pub engine_at: i64,
    /// From the inbox or from the catch-up.
    pub source: Source,
}

impl std::fmt::Debug for NewEvent<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewEvent")
            .field("kind", &self.kind)
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

/// Writes an event, once. Returns its row, or `None` when the same event was already there.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses; [`RepoError::Vault`] when the
/// generator of the system fails.
pub async fn record(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    event: &NewEvent<'_>,
    now: i64,
) -> Result<Option<Uuid>, RepoError> {
    let id = Uuid::new_v4();
    let digest = key.whatsapp_digest(WhatsappField::EventKey, event.engine, event.key);
    let body_enc = key.seal_whatsapp(WhatsappField::EventBody, id, event.body)?;
    Ok(sqlx::query_scalar(
        "INSERT INTO engine_event (id, organization_id, engine_id, number_id, key_digest, kind, \
                                   source, body_enc, engine_at, received_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (engine_id, key_digest) DO NOTHING RETURNING id",
    )
    .bind(id)
    .bind(organization.id())
    .bind(event.engine)
    .bind(event.number)
    .bind(digest.as_slice())
    .bind(event.kind.as_str())
    .bind(event.source.as_str())
    .bind(body_enc)
    .bind(event.engine_at)
    .bind(now)
    .fetch_optional(connection)
    .await?)
}

/// Marks an event as handled.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn handled(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    event: Uuid,
    now: i64,
) -> Result<(), RepoError> {
    sqlx::query(
        "UPDATE engine_event SET processed_at = $3 \
         WHERE organization_id = $1 AND id = $2 AND processed_at IS NULL",
    )
    .bind(organization.id())
    .bind(event)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

/// An event still to be handled, without its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenEvent {
    /// The row.
    pub id: Uuid,
    /// The number of the session.
    pub number: Uuid,
    /// The kind.
    pub kind: EventKind,
    /// When, in milliseconds of the engine.
    pub engine_at: i64,
    /// When the hub wrote it.
    pub received_at: i64,
}

/// The events still to be handled, oldest first: what the queue of notices reads.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn open(
    pool: &PgPool,
    organization: ResolvedOrg,
    limit: u32,
) -> Result<Vec<OpenEvent>, RepoError> {
    let rows: Vec<(Uuid, Uuid, String, i64, i64)> = sqlx::query_as(
        "SELECT id, number_id, kind, engine_at, received_at FROM engine_event \
         WHERE organization_id = $1 AND processed_at IS NULL \
         ORDER BY received_at, id LIMIT $2",
    )
    .bind(organization.id())
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(id, number, kind, engine_at, received_at)| {
            Ok(OpenEvent {
                id,
                number,
                kind: EventKind::parse(&kind)
                    .ok_or(RepoError::Inconsistent("the kind of an event"))?,
                engine_at,
                received_at,
            })
        })
        .collect()
}

/// The body of the event `event`, opened.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses; [`RepoError::Inconsistent`] when it
/// does not open.
pub async fn body(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    event: Uuid,
) -> Result<Option<Zeroizing<Vec<u8>>>, RepoError> {
    let block: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT body_enc FROM engine_event WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(event)
    .fetch_optional(pool)
    .await?;
    block
        .map(|block| {
            key.open_whatsapp(WhatsappField::EventBody, event, &block)
                .map_err(|_| RepoError::Inconsistent("the body of an event does not open"))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_is_one_the_schema_accepts() {
        const MIGRATION: &str = include_str!("../../migrations/0010_whatsapp.sql");
        for kind in EventKind::ALL {
            assert_eq!(EventKind::parse(kind.as_str()), Some(*kind));
            assert!(
                MIGRATION.contains(&format!("'{}'", kind.as_str())),
                "{kind:?}"
            );
        }
        let events: Vec<&str> = EventKind::ALL.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(events, crate::engine::waha::EVENTS);
    }
}
