//! The messages a Hermes sent through Arauto (decision 0054, section 7), so that a reading of
//! a chat can say who sent each message the number wrote.
//!
//! The identifier WhatsApp gave the message is kept only as a keyed digest
//! ([`WhatsappField::SentMessageId`]), never in the clear. The Hermes that sent it goes to
//! null when its row is deleted. A row is purged by its own `purge_at`, written with it: the
//! sending plus the days of `retention` ([`super::purge`]).

use std::collections::HashMap;

use arauto_protocol::limits::DAY_MS;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::RepoError;
use crate::db::ResolvedOrg;
use crate::engine::EngineMessageId;
use crate::vault::{OrganizationKey, WhatsappField};

/// The days a record is kept when `retention` has no row for it.
const DAYS_DEFAULT: i64 = 30;

/// Who sent a message the number wrote, as a reading tells the Hermes that reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SentBy {
    /// The Hermes that reads sent it.
    YouThroughArauto,
    /// Another Hermes, or one that was removed. Its name is never given.
    AnotherHermesThroughArauto,
    /// No record: sent from the app or another bridge, older than the record, or a sending
    /// whose identifier the engine did not return.
    OutsideArauto,
}

impl SentBy {
    /// The word a reading shows.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            SentBy::YouThroughArauto => "you_through_arauto",
            SentBy::AnotherHermesThroughArauto => "another_hermes_through_arauto",
            SentBy::OutsideArauto => "outside_arauto",
        }
    }
}

fn digest(key: &OrganizationKey, number: Uuid, message: &EngineMessageId) -> [u8; 32] {
    key.whatsapp_digest(
        WhatsappField::SentMessageId,
        number,
        message.whatsapp_id().as_bytes(),
    )
}

/// Records that `hermes` sent `message` in `chat` of `number`, at `now`. Sending the same
/// message twice is one row.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
#[allow(clippy::too_many_arguments)]
pub async fn record(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    chat: Uuid,
    hermes: Uuid,
    message: &EngineMessageId,
    now: i64,
) -> Result<(), RepoError> {
    let days: Option<i32> =
        sqlx::query_scalar("SELECT days FROM retention WHERE table_name = 'sent_message'")
            .fetch_optional(pool)
            .await?;
    let days = days.map_or(DAYS_DEFAULT, i64::from);
    sqlx::query(
        "INSERT INTO sent_message (id, organization_id, number_id, chat_id, hermes_id, \
                                   id_digest, sent_at, purge_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (number_id, id_digest) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(organization.id())
    .bind(number)
    .bind(chat)
    .bind(hermes)
    .bind(digest(key, number, message).as_slice())
    .bind(now)
    .bind(now.saturating_add(days.saturating_mul(DAY_MS)))
    .execute(pool)
    .await?;
    Ok(())
}

/// Who sent each of `messages` (in `number`), as seen by `reader`: the position of each one
/// that has a record. A message without a record is not in the map.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn labels(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    reader: Uuid,
    messages: &[&EngineMessageId],
) -> Result<HashMap<usize, SentBy>, RepoError> {
    if messages.is_empty() {
        return Ok(HashMap::new());
    }
    let digests: Vec<[u8; 32]> = messages
        .iter()
        .map(|message| digest(key, number, message))
        .collect();
    let bound: Vec<Vec<u8>> = digests.iter().map(|digest| digest.to_vec()).collect();
    let rows: Vec<(Vec<u8>, Option<Uuid>)> = sqlx::query_as(
        "SELECT id_digest, hermes_id FROM sent_message \
         WHERE organization_id = $1 AND number_id = $2 AND id_digest = ANY($3)",
    )
    .bind(organization.id())
    .bind(number)
    .bind(&bound)
    .fetch_all(pool)
    .await?;
    let mut labels = HashMap::new();
    for (found, hermes) in rows {
        let label = if hermes == Some(reader) {
            SentBy::YouThroughArauto
        } else {
            SentBy::AnotherHermesThroughArauto
        };
        for (index, wanted) in digests.iter().enumerate() {
            if wanted.as_slice() == found.as_slice() {
                labels.insert(index, label);
            }
        }
    }
    Ok(labels)
}
