//! The short-lived addresses of the files of messages (decision 0055): what
//! `whatsapp_media_link` writes, and what `GET /v1/media/{token}` reads.
//!
//! The address is a short-lived secret. Only its digest is kept ([`MediaDigest`]), with the
//! organization, the number, the chat, the Hermes it was given to, the identifier of the message
//! in the engine (sealed with the key of the organization and bound to the row,
//! [`WhatsappField::MediaLinkMessage`]) and the two times. A reading of the address reads the
//! row and writes nothing: no count of the openings, no new end. The row ends at `expires_at`,
//! and the purge removes it one day later ([`super::purge`]).

use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::RepoError;
use crate::db::ResolvedOrg;
use crate::engine::EngineMessageId;
use crate::media::MediaDigest;
use crate::vault::{OrganizationKey, WhatsappField};

/// What a new address names.
#[derive(Debug, Clone)]
pub struct NewLink<'a> {
    /// The number of the message.
    pub number: Uuid,
    /// The chat of the hub the message is in.
    pub chat: Uuid,
    /// The Hermes the address is given to.
    pub hermes: Uuid,
    /// The message, as the engine knows it.
    pub message: &'a EngineMessageId,
}

/// What an address that still serves names.
#[derive(Debug, Clone)]
pub struct Link {
    /// The number of the message.
    pub number: Uuid,
    /// The chat of the hub the message is in.
    pub chat: Uuid,
    /// The Hermes the address was given to.
    pub hermes: Uuid,
    /// The message, as the engine knows it.
    pub message: EngineMessageId,
}

/// Records an address of `link`, by its digest, from `now` to `expires_at`.
///
/// # Errors
/// [`RepoError::Vault`] when the generator of the system fails; [`RepoError::Database`] when
/// the database refuses.
pub async fn issue(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    link: &NewLink<'_>,
    digest: &MediaDigest,
    now: i64,
    expires_at: i64,
) -> Result<(), RepoError> {
    let id = Uuid::new_v4();
    let message_enc = key.seal_whatsapp(
        WhatsappField::MediaLinkMessage,
        id,
        link.message.as_str().as_bytes(),
    )?;
    sqlx::query(
        "INSERT INTO media_link (id, organization_id, number_id, chat_id, hermes_id, \
                                 message_enc, token_digest, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(id)
    .bind(organization.id())
    .bind(link.number)
    .bind(link.chat)
    .bind(link.hermes)
    .bind(message_enc)
    .bind(digest.as_bytes().as_slice())
    .bind(now)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// The address of this digest, while it serves (`now` before its end). One read-only `SELECT`:
/// nothing is written, whatever is found.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses; [`RepoError::Inconsistent`] when the
/// sealed message does not open.
pub async fn find(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    digest: &MediaDigest,
    now: i64,
) -> Result<Option<Link>, RepoError> {
    let row: Option<(Uuid, Uuid, Uuid, Uuid, Vec<u8>)> = sqlx::query_as(
        "SELECT id, number_id, chat_id, hermes_id, message_enc FROM media_link \
         WHERE organization_id = $1 AND token_digest = $2 AND expires_at > $3",
    )
    .bind(organization.id())
    .bind(digest.as_bytes().as_slice())
    .bind(now)
    .fetch_optional(pool)
    .await?;
    let Some((id, number, chat, hermes, message_enc)) = row else {
        return Ok(None);
    };
    let opened = key
        .open_whatsapp(WhatsappField::MediaLinkMessage, id, &message_enc)
        .map_err(|_| RepoError::Inconsistent("the message of a media address does not open"))?;
    let text = std::str::from_utf8(&opened)
        .map_err(|_| RepoError::Inconsistent("the message of a media address is not text"))?;
    let message = EngineMessageId::parse(text)
        .map_err(|_| RepoError::Inconsistent("the message of a media address"))?;
    Ok(Some(Link {
        number,
        chat,
        hermes,
        message,
    }))
}
