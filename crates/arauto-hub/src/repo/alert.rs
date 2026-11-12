//! Alerts: what the panel would show the admin (`docs/07-telas.md`, the alert strip). In the
//! MVP they are read by `arauto-hub admin alerts`.
//!
//! The catalog of kinds is closed, and so is what an alert can say: fixed words, numbers,
//! identifiers and addresses. No text that came from a device goes in.

use std::net::IpAddr;

use sqlx::postgres::{PgConnection, PgPool};
use sqlx::types::Json;
use uuid::Uuid;

use super::RepoError;
use crate::db::ResolvedOrg;

macro_rules! alert_kinds {
    ($( $(#[$meta:meta])* $variant:ident => $text:literal, )+) => {
        /// What an alert is about. The catalog is closed.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum AlertKind {
            $( $(#[$meta])* $variant, )+
        }

        impl AlertKind {
            /// Every kind, in the order of the catalog.
            pub const ALL: &'static [AlertKind] = &[$(AlertKind::$variant,)+];

            /// The text stored in `alert.kind`.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(AlertKind::$variant => $text,)+
                }
            }

            /// Reads the stored text.
            #[must_use]
            pub fn parse(text: &str) -> Option<Self> {
                match text {
                    $($text => Some(AlertKind::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

alert_kinds! {
    /// Another identity presented an invite whose request is still pending: the block went
    /// somewhere else (C3).
    InviteMisused => "invite_misused",
    /// Two waiting connections with the same identity: it was copied before the approval (C6).
    IdentityCopied => "identity_copied",
    /// The hub stopped trusting the credential of a device (C9, C10).
    Quarantined => "quarantined",
    /// The admin did not recognize the last use of the device being replaced (C10).
    CopySuspected => "copy_suspected",
    /// A session that was being served was taken by another connection (C9).
    ConnectionTaken => "connection_taken",
    /// A device asked for its own closing (C10).
    DeviceLeft => "device_left",
    /// A closed device presented a seal older than its last ones (C7).
    ReuseAfterClosing => "reuse_after_closing",
    /// Three connections in 24 hours received a credential and ended without the "saved" (C7).
    UnconfirmedSwaps => "unconfirmed_swaps",
    /// A security event is committed and the audit mark did not follow it: until the next
    /// event moves the mark, a restoration that loses that event is not noticed (C11).
    MarkNotAdvanced => "mark_not_advanced",
    /// The engine is of a version the hub was not checked against (`referencia/waha-fatos.md`,
    /// the rule for new versions).
    EngineVersionUntested => "engine_version_untested",
    /// A session of the engine had another configuration, for instance another destination
    /// for its events, and the hub applied its own (`docs/05-modelo-de-ameacas.md`, 19).
    EngineSessionCorrected => "engine_session_corrected",
    /// The telephone that paired a number is not the one that was added.
    NumberSwapped => "number_swapped",
}

impl std::fmt::Display for AlertKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What else an alert says. Every key is fixed in the program; every value is a fixed word, a
/// number, an identifier or an address.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct AlertDetails(serde_json::Map<String, serde_json::Value>);

impl AlertDetails {
    /// No details.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// A word fixed in the program: a reason.
    #[must_use]
    pub(crate) fn word(mut self, key: &'static str, value: &'static str) -> Self {
        self.0.insert(key.to_owned(), value.into());
        self
    }

    /// A number: a count, a time in milliseconds.
    #[must_use]
    pub(crate) fn number(mut self, key: &'static str, value: i64) -> Self {
        self.0.insert(key.to_owned(), value.into());
        self
    }

    /// A number that may be missing.
    #[must_use]
    pub(crate) fn maybe_number(self, key: &'static str, value: Option<i64>) -> Self {
        match value {
            Some(value) => self.number(key, value),
            None => self,
        }
    }

    /// An address, when there is one.
    #[must_use]
    pub(crate) fn address(mut self, key: &'static str, value: Option<IpAddr>) -> Self {
        if let Some(value) = value {
            self.0.insert(key.to_owned(), value.to_string().into());
        }
        self
    }

    /// The identifier of a row.
    #[must_use]
    pub(crate) fn id(mut self, key: &'static str, value: Uuid) -> Self {
        self.0
            .insert(key.to_owned(), value.as_hyphenated().to_string().into());
        self
    }
}

/// An alert to be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewAlert {
    pub(crate) kind: AlertKind,
    pub(crate) hermes: Option<Uuid>,
    pub(crate) device: Option<Uuid>,
    pub(crate) details: AlertDetails,
}

/// Writes an alert in the transaction of the change that caused it.
pub(crate) async fn raise(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    at: i64,
    alert: &NewAlert,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO alert (id, organization_id, kind, hermes_id, device_id, at, details) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(organization.id())
    .bind(alert.kind.as_str())
    .bind(alert.hermes)
    .bind(alert.device)
    .bind(at)
    .bind(Json(serde_json::Value::Object(alert.details.0.clone())))
    .execute(connection)
    .await?;
    Ok(id)
}

/// Whether an alert of this kind about this device was raised after `since`. It keeps a
/// repeated presentation from filling the table.
pub(crate) async fn raised_since(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    kind: AlertKind,
    device: Uuid,
    since: i64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM alert WHERE organization_id = $1 AND kind = $2 \
                        AND device_id = $3 AND at > $4)",
    )
    .bind(organization.id())
    .bind(kind.as_str())
    .bind(device)
    .bind(since)
    .fetch_one(connection)
    .await
}

/// A security event of a connection of `device` is committed and the audit mark did not
/// follow it. The hub closed that connection and tells the admin, with the number of the
/// event.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn mark_not_advanced(
    pool: &PgPool,
    organization: ResolvedOrg,
    device: Option<Uuid>,
    seq: i64,
    now: i64,
) -> Result<(), RepoError> {
    let mut connection = pool.acquire().await?;
    let hermes: Option<Uuid> = match device {
        Some(device) => {
            sqlx::query_scalar(
                "SELECT hermes_id FROM device WHERE organization_id = $1 AND id = $2",
            )
            .bind(organization.id())
            .bind(device)
            .fetch_optional(&mut *connection)
            .await?
        }
        None => None,
    };
    raise(
        &mut connection,
        organization,
        now,
        &NewAlert {
            kind: AlertKind::MarkNotAdvanced,
            hermes,
            device,
            details: AlertDetails::new().number("seq", seq),
        },
    )
    .await?;
    Ok(())
}

/// An alert, as the admin reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    /// The identifier, for the acknowledgment.
    pub id: Uuid,
    /// What it is about.
    pub kind: AlertKind,
    /// The Hermes it is about.
    pub hermes_id: Option<Uuid>,
    /// The name of that Hermes.
    pub hermes_name: Option<String>,
    /// The device it is about.
    pub device_id: Option<Uuid>,
    /// When it was raised.
    pub at: i64,
    /// What else it says: fixed words, numbers, identifiers and addresses.
    pub details: serde_json::Value,
    /// When the admin marked it as seen.
    pub seen_at: Option<i64>,
}

/// Which alerts to list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertFilter {
    /// The ones nobody marked as seen.
    Unseen,
    /// All of them.
    All,
}

type AlertRow = (
    Uuid,
    String,
    Option<Uuid>,
    Option<String>,
    Option<Uuid>,
    i64,
    Json<serde_json::Value>,
    Option<i64>,
);

/// The alerts of `organization`, newest first, at most `limit`.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses; [`RepoError::Inconsistent`] when a row
/// has a kind this program does not know.
pub async fn list(
    pool: &PgPool,
    organization: ResolvedOrg,
    filter: AlertFilter,
    limit: u32,
) -> Result<Vec<Alert>, RepoError> {
    let rows: Vec<AlertRow> = sqlx::query_as(
        "SELECT a.id, a.kind, a.hermes_id, h.name, a.device_id, a.at, a.details, a.seen_at \
         FROM alert a \
         LEFT JOIN hermes h ON h.organization_id = a.organization_id AND h.id = a.hermes_id \
         WHERE a.organization_id = $1 AND ($2 OR a.seen_at IS NULL) \
         ORDER BY a.at DESC, a.id \
         LIMIT $3",
    )
    .bind(organization.id())
    .bind(filter == AlertFilter::All)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(
            |(id, kind, hermes_id, hermes_name, device_id, at, details, seen_at)| {
                Ok(Alert {
                    id,
                    kind: AlertKind::parse(&kind)
                        .ok_or(RepoError::Inconsistent("an alert of an unknown kind"))?,
                    hermes_id,
                    hermes_name,
                    device_id,
                    at,
                    details: details.0,
                    seen_at,
                })
            },
        )
        .collect()
}

/// Marks an alert as seen. Returns whether an unseen alert with this identifier existed in
/// `organization`.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn acknowledge(
    pool: &PgPool,
    organization: ResolvedOrg,
    alert: Uuid,
    now: i64,
) -> Result<bool, RepoError> {
    let changed = sqlx::query(
        "UPDATE alert SET seen_at = $3 \
         WHERE organization_id = $1 AND id = $2 AND seen_at IS NULL",
    )
    .bind(organization.id())
    .bind(alert)
    .bind(now)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed == 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_kind_has_its_own_text_and_reads_back() {
        let texts: BTreeSet<&str> = AlertKind::ALL.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(texts.len(), AlertKind::ALL.len());
        for kind in AlertKind::ALL {
            assert_eq!(AlertKind::parse(kind.as_str()), Some(*kind));
            assert_eq!(kind.to_string(), kind.as_str());
        }
        assert_eq!(AlertKind::parse("anything_else"), None);
    }

    #[test]
    fn details_hold_only_what_was_given() {
        let details = AlertDetails::new()
            .word("reason", "reuse")
            .number("count", 3)
            .maybe_number("last_use_at", None)
            .address("from", Some("192.0.2.7".parse().unwrap()))
            .address("last_use_from", None)
            .id("request", Uuid::from_u128(9));
        assert_eq!(
            serde_json::Value::Object(details.0),
            serde_json::json!({
                "reason": "reuse",
                "count": 3,
                "from": "192.0.2.7",
                "request": "00000000-0000-0000-0000-000000000009",
            })
        );
    }
}
