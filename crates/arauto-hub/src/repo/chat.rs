//! The chats of a number (`docs/06-modelo-de-dados.md`, section 4).
//!
//! A chat has an opaque identifier of the hub (a UUID): that is what a Hermes and a notice
//! see. The identifiers it has in the engine carry telephones, so each is kept encrypted, with
//! a keyed digest to find it; a chat may have more than one.

use sqlx::postgres::{PgConnection, PgPool};
use uuid::Uuid;

use super::RepoError;
use crate::db::ResolvedOrg;
use crate::engine::EngineChatId;
use crate::vault::{OrganizationKey, WhatsappField};

/// The chat of `number` that the engine knows as `engine_chat`, if the hub has it.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn find(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    engine_chat: &EngineChatId,
) -> Result<Option<Uuid>, RepoError> {
    let digest = key.whatsapp_digest(
        WhatsappField::ChatEngineId,
        number,
        engine_chat.as_str().as_bytes(),
    );
    Ok(sqlx::query_scalar(
        "SELECT chat_id FROM chat_engine_id \
         WHERE organization_id = $1 AND number_id = $2 AND id_digest = $3",
    )
    .bind(organization.id())
    .bind(number)
    .bind(digest.as_slice())
    .fetch_optional(pool)
    .await?)
}

/// Whether the engine names this chat by a hidden identifier instead of a telephone. The
/// same person may show up as a telephone in one place and as a hidden identifier in
/// another, and the hub has no way to tell that the two are one: such a chat is a chat of
/// its own, and it is marked.
fn is_hidden(engine_chat: &EngineChatId) -> bool {
    engine_chat.as_str().ends_with("@lid")
}

/// The chat of `number` that the engine knows as `engine_chat`, created when the hub did not
/// have it yet. Two calls at once give the same chat.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn resolve(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    engine_chat: &EngineChatId,
    now: i64,
) -> Result<Uuid, RepoError> {
    let digest = key.whatsapp_digest(
        WhatsappField::ChatEngineId,
        number,
        engine_chat.as_str().as_bytes(),
    );
    let found: Option<Uuid> = sqlx::query_scalar(
        "SELECT chat_id FROM chat_engine_id \
         WHERE organization_id = $1 AND number_id = $2 AND id_digest = $3",
    )
    .bind(organization.id())
    .bind(number)
    .bind(digest.as_slice())
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(found) = found {
        return Ok(found);
    }
    let chat = Uuid::new_v4();
    let id_enc = key.seal_whatsapp(
        WhatsappField::ChatEngineId,
        chat,
        engine_chat.as_str().as_bytes(),
    )?;
    sqlx::query(
        "INSERT INTO chat (id, organization_id, number_id, kind, created_at, hidden_id) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(chat)
    .bind(organization.id())
    .bind(number)
    .bind(engine_chat.kind().as_str())
    .bind(now)
    .bind(is_hidden(engine_chat))
    .execute(&mut *connection)
    .await?;
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO chat_engine_id (organization_id, number_id, chat_id, id_digest, id_enc, \
                                     created_at) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (number_id, id_digest) DO NOTHING RETURNING chat_id",
    )
    .bind(organization.id())
    .bind(number)
    .bind(chat)
    .bind(digest.as_slice())
    .bind(id_enc)
    .bind(now)
    .fetch_optional(&mut *connection)
    .await?;
    if inserted.is_some() {
        return Ok(chat);
    }
    // Someone else created it meanwhile: theirs is the chat, and the empty one goes away.
    sqlx::query("DELETE FROM chat WHERE organization_id = $1 AND id = $2")
        .bind(organization.id())
        .bind(chat)
        .execute(&mut *connection)
        .await?;
    sqlx::query_scalar(
        "SELECT chat_id FROM chat_engine_id \
         WHERE organization_id = $1 AND number_id = $2 AND id_digest = $3",
    )
    .bind(organization.id())
    .bind(number)
    .bind(digest.as_slice())
    .fetch_one(&mut *connection)
    .await
    .map_err(RepoError::from)
}

/// The identifiers the engine knows the chat `chat` of `number` by, oldest first. Empty when
/// the chat is not of that number.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses; [`RepoError::Inconsistent`] when a
/// stored identifier does not open.
pub async fn engine_ids(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    chat: Uuid,
) -> Result<Vec<EngineChatId>, RepoError> {
    let rows: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT id_enc FROM chat_engine_id \
         WHERE organization_id = $1 AND number_id = $2 AND chat_id = $3 ORDER BY created_at",
    )
    .bind(organization.id())
    .bind(number)
    .bind(chat)
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|block| {
            let opened = key
                .open_whatsapp(WhatsappField::ChatEngineId, chat, block)
                .map_err(|_| RepoError::Inconsistent("an identifier of a chat does not open"))?;
            let text = std::str::from_utf8(&opened)
                .map_err(|_| RepoError::Inconsistent("an identifier of a chat is not text"))?;
            EngineChatId::parse(text)
                .map_err(|_| RepoError::Inconsistent("an identifier of a chat"))
        })
        .collect()
}

/// [`find`], inside a transaction.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn find_in(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    engine_chat: &EngineChatId,
) -> Result<Option<Uuid>, RepoError> {
    let digest = key.whatsapp_digest(
        WhatsappField::ChatEngineId,
        number,
        engine_chat.as_str().as_bytes(),
    );
    Ok(sqlx::query_scalar(
        "SELECT chat_id FROM chat_engine_id \
         WHERE organization_id = $1 AND number_id = $2 AND id_digest = $3",
    )
    .bind(organization.id())
    .bind(number)
    .bind(digest.as_slice())
    .fetch_optional(connection)
    .await?)
}

/// The chats of `number` the hub already has for each of these identifiers, in one query: the
/// position of each identifier found, and its chat.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn find_many(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    number: Uuid,
    engine_chats: &[EngineChatId],
) -> Result<std::collections::HashMap<usize, Uuid>, RepoError> {
    let digests: Vec<[u8; 32]> = engine_chats
        .iter()
        .map(|chat| {
            key.whatsapp_digest(
                WhatsappField::ChatEngineId,
                number,
                chat.as_str().as_bytes(),
            )
        })
        .collect();
    if digests.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let bound: Vec<Vec<u8>> = digests.iter().map(|digest| digest.to_vec()).collect();
    let rows: Vec<(Vec<u8>, Uuid)> = sqlx::query_as(
        "SELECT id_digest, chat_id FROM chat_engine_id \
         WHERE organization_id = $1 AND number_id = $2 AND id_digest = ANY($3)",
    )
    .bind(organization.id())
    .bind(number)
    .bind(&bound)
    .fetch_all(pool)
    .await?;
    let mut found = std::collections::HashMap::new();
    for (digest, chat) in rows {
        for (index, wanted) in digests.iter().enumerate() {
            if wanted.as_slice() == digest.as_slice() {
                found.insert(index, chat);
            }
        }
    }
    Ok(found)
}
