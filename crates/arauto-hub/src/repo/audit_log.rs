//! Reading the audit. Writing it is in [`crate::audit`]; nothing here changes a row.

use sqlx::postgres::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

use super::RepoError;
use crate::db::ResolvedOrg;

/// One row of the audit, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
    /// The number of the event.
    pub seq: i64,
    /// Whether it belongs to the organization, or to the whole installation.
    pub of_organization: bool,
    /// When it happened, in milliseconds.
    pub at: i64,
    /// `admin`, `device` or `system`.
    pub actor_kind: String,
    /// The row of the actor.
    pub actor_id: Option<Uuid>,
    /// The system user of a local admin. Declared, not verified: show it escaped.
    pub actor_note: Option<String>,
    /// What happened: a word of the closed catalog.
    pub action: String,
    /// The kind of thing it is about.
    pub target_kind: Option<String>,
    /// The thing it is about.
    pub target_id: Option<Uuid>,
    /// The address it came from.
    pub address: Option<String>,
    /// `ok`, `denied` or `error`.
    pub result: String,
    /// The code that was answered.
    pub code: Option<String>,
    /// Whether it is a security event.
    pub security: bool,
    /// What else it says: numbers, flags, identifiers, names and fixed words.
    pub details: serde_json::Value,
}

type Row = (
    i64,
    bool,
    i64,
    String,
    Option<Uuid>,
    Option<String>,
    String,
    Option<String>,
    Option<Uuid>,
    Option<String>,
    String,
    Option<String>,
    bool,
    Json<serde_json::Value>,
);

/// The last `count` events of `organization` and of the installation it runs in (the
/// restored mode, the refusals that belong to no organization), oldest first.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn tail(
    pool: &PgPool,
    organization: ResolvedOrg,
    count: u32,
) -> Result<Vec<AuditRow>, RepoError> {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT seq, organization_id IS NOT NULL, at, actor_kind, actor_id, actor_note, action, \
                target_kind, target_id, address, result, code, security, details \
         FROM audit \
         WHERE organization_id = $1 OR organization_id IS NULL \
         ORDER BY seq DESC \
         LIMIT $2",
    )
    .bind(organization.id())
    .bind(i64::from(count))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .rev()
        .map(
            |(
                seq,
                of_organization,
                at,
                actor_kind,
                actor_id,
                actor_note,
                action,
                target_kind,
                target_id,
                address,
                result,
                code,
                security,
                details,
            )| AuditRow {
                seq,
                of_organization,
                at,
                actor_kind,
                actor_id,
                actor_note,
                action,
                target_kind,
                target_id,
                address,
                result,
                code,
                security,
                details: details.0,
            },
        )
        .collect())
}
