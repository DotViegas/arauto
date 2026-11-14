//! The restored mode (`docs/04-seguranca-conexao.md`, C11).
//!
//! A hub that comes back from an older copy of its database forgets what happened after the
//! copy: approvals, swaps, quarantines, revocations. It notices in two ways:
//!
//! - the **audit mark**, a number outside the database: a database whose last security event
//!   is behind it, or a database with data beside no mark at all ([`check`], and the same
//!   check inside every security write);
//! - a **valid seal the database does not explain**, which the swap rule reports
//!   ([`crate::core::judge::Decision::RestorationProof`]).
//!
//! Either way the answer is [`enter`]: in **one** transaction, every active device is
//! suspended, issued invites are cancelled, requests in progress are closed, every existing
//! device is marked, the numbering of the audit continues from the mark and the first new
//! event is written. Only after the commit does the mark advance, so starting again does not
//! accuse the same restoration twice.
//!
//! There is no "reactivate": each Hermes gets out by reconnecting or by being revoked, and
//! the mode ends when no suspended device is left ([`end_when_done`]).

use std::fmt;
use std::net::IpAddr;

use arauto_protocol::message::DeclaredText;
use arauto_protocol::noise::PublicKey;
use sqlx::postgres::{PgConnection, PgPool};
use uuid::Uuid;

use crate::audit::{self, Action, Actor, AuditError, Details, Event, TargetKind};
use crate::db::{self, DbError, ResolvedOrg};
use crate::mark::{Mark, MarkError, MarkValue};

/// Why the hub concluded that its database came back from an older copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// The last security event of the database is behind the mark.
    BehindMark {
        /// The mark.
        mark: i64,
        /// The number of the last security event in the database; zero when there is none.
        database: i64,
    },
    /// The database has data and there is no mark.
    MarkAbsent,
    /// A known device presented a valid seal of a generation above every one the database
    /// knows of it.
    SealBeyondDatabase {
        /// The device.
        device: Uuid,
    },
    /// An identity the database does not know presented a valid seal.
    UnknownDeviceSeal {
        /// Its row in the list of unknown devices.
        unknown_device: Uuid,
    },
}

impl Evidence {
    /// The word that goes into the audit.
    const fn word(&self) -> &'static str {
        match self {
            Evidence::BehindMark { .. } => "behind_mark",
            Evidence::MarkAbsent => "mark_absent",
            Evidence::SealBeyondDatabase { .. } => "seal_beyond_database",
            Evidence::UnknownDeviceSeal { .. } => "unknown_device_seal",
        }
    }
}

impl fmt::Display for Evidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Evidence::BehindMark { mark, database } => write!(
                f,
                "the last security event of the database is {database} and the audit mark is \
                 at {mark}"
            ),
            Evidence::MarkAbsent => f.write_str("the database has data and there is no audit mark"),
            Evidence::SealBeyondDatabase { .. } => f.write_str(
                "a device presented a valid credential newer than any the database knows",
            ),
            Evidence::UnknownDeviceSeal { .. } => {
                f.write_str("a device the database does not know presented a valid credential")
            }
        }
    }
}

/// How the database stands against the mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// No mark and no data: `init` has not run.
    NotInstalled,
    /// The database is not behind the mark.
    Consistent,
    /// The database came back from an older copy.
    Restored(Evidence),
}

/// What went wrong in this module.
#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    /// The database refused.
    #[error(transparent)]
    Database(#[from] DbError),

    /// The mark could not be read.
    #[error(transparent)]
    Mark(#[from] MarkError),

    /// A security write refused or failed.
    #[error(transparent)]
    Audit(#[from] AuditError),

    /// There is no installation in this database.
    #[error("the hub is not installed: run `arauto-hub init`")]
    NotInstalled,

    /// The evidence names a device that is not in the database.
    #[error("the device of the evidence is not in the database")]
    NoSuchDevice,

    /// The restored mode **is on and committed**, and the mark could not follow.
    ///
    /// When there was a mark, the next security event moves it; until then a start reads the
    /// database as ahead of the mark, which accuses nothing. When the mark was **absent**, it
    /// still is, and the database is accused again: calling [`enter`] once more only creates
    /// the mark at this event and answers [`EnterOutcome::AlreadyHandled`], without turning
    /// the mode on a second time.
    #[error("the restored mode is on (event {seq}), but the audit mark did not advance: {cause}")]
    MarkNotAdvanced {
        /// The number of the event that turned the mode on.
        seq: i64,
        /// Why the mark was not written.
        cause: MarkError,
    },
}

impl From<sqlx::Error> for RestoreError {
    fn from(error: sqlx::Error) -> Self {
        RestoreError::Database(DbError::Query(error))
    }
}

/// Compares the database with a mark that was read. The one place where "behind the mark"
/// is decided, for the start, for the sweep and for every security write.
pub(crate) async fn standing(
    connection: &mut PgConnection,
    marked: MarkValue,
) -> Result<Standing, sqlx::Error> {
    let (last_security, has_data): (Option<i64>, bool) = sqlx::query_as(
        "SELECT (SELECT seq FROM audit WHERE security ORDER BY seq DESC LIMIT 1), \
                EXISTS (SELECT 1 FROM installation) OR EXISTS (SELECT 1 FROM audit)",
    )
    .fetch_one(connection)
    .await?;
    Ok(match marked {
        MarkValue::Absent if has_data => Standing::Restored(Evidence::MarkAbsent),
        MarkValue::Absent => Standing::NotInstalled,
        MarkValue::At(mark) => {
            let database = last_security.unwrap_or(0);
            if database < mark {
                Standing::Restored(Evidence::BehindMark { mark, database })
            } else {
                Standing::Consistent
            }
        }
    })
}

/// The check of the start and of every sweep: reads the mark and compares the database with
/// it. Changes nothing.
///
/// # Errors
/// [`RestoreError::Mark`] when the mark is damaged or cannot be read;
/// [`RestoreError::Database`] when the database refuses.
pub async fn check(pool: &PgPool, mark: &Mark) -> Result<Standing, RestoreError> {
    let marked = mark.read().await?;
    let mut connection = pool.acquire().await?;
    Ok(standing(&mut connection, marked).await?)
}

/// The row of `installation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Installation {
    /// When `init` ran.
    pub created_at: i64,
    /// Whether the restored mode is on.
    pub restored_mode: bool,
    /// When the mode began.
    pub restored_at: Option<i64>,
    /// The time of the last event of the database that was restored: what the admin is told
    /// the copy is from.
    pub restored_copy_of: Option<i64>,
}

/// Reads the installation, if `init` already created it.
///
/// # Errors
/// [`RestoreError::Database`] when the database refuses.
pub async fn installation(pool: &PgPool) -> Result<Option<Installation>, RestoreError> {
    let row: Option<(i64, bool, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT created_at, restored_mode, restored_at, restored_copy_of FROM installation",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(created_at, restored_mode, restored_at, restored_copy_of)| Installation {
            created_at,
            restored_mode,
            restored_at,
            restored_copy_of,
        },
    ))
}

/// What turning the mode on did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entered {
    /// The number of the first new event, above the mark.
    pub seq: i64,
    /// Active devices that were suspended. Their open sessions must be closed.
    pub suspended: u64,
    /// Requests in progress (pending or approved) that were closed.
    pub requests_closed: u64,
    /// Issued invites that were cancelled.
    pub invites_cancelled: u64,
    /// Devices that were marked now.
    pub marked: u64,
}

/// The outcome of [`enter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnterOutcome {
    /// The mode was turned on, or turned on again for a new restoration.
    Entered(Entered),
    /// The evidence no longer holds: another task or process already handled it.
    AlreadyHandled,
}

/// Turns the restored mode on, or on again when it already was.
///
/// One transaction, committed durably:
///
/// - every active device becomes suspended;
/// - issued invites are cancelled, and pending and approved requests are closed (a request
///   already in quarantine stays as it is);
/// - every device that exists, in any state, is marked;
/// - the numbering of the audit continues from the mark, and the first new event is written.
///
/// Only then does the mark advance (it is created again when it was absent). The caller
/// closes the open sessions.
///
/// The evidence is looked at again under the lock of the installation: a database that is no
/// longer behind the mark, a device that was marked meanwhile, or, for an unknown device, a
/// mode that is already on or a listing whose proof was already handled, give
/// [`EnterOutcome::AlreadyHandled`].
///
/// A mark that is absent beside a database whose last security event is the one that turned
/// the mode on is a mark that was lost on the way of that very write: it is created again at
/// that event and nothing else is done.
///
/// # Errors
/// [`RestoreError::NotInstalled`] when there is no installation;
/// [`RestoreError::NoSuchDevice`] when the evidence names a device that does not exist;
/// [`RestoreError::MarkNotAdvanced`] when everything was committed and only the mark failed.
pub async fn enter(
    pool: &PgPool,
    mark: &Mark,
    now: i64,
    evidence: Evidence,
) -> Result<EnterOutcome, RestoreError> {
    let mut transaction = db::begin_durable(pool).await?;
    let mode_on: Option<bool> =
        sqlx::query_scalar("SELECT restored_mode FROM installation FOR UPDATE")
            .fetch_optional(&mut *transaction)
            .await?;
    let Some(mode_on) = mode_on else {
        return Err(RestoreError::NotInstalled);
    };
    // Read after the lock: whoever held it before may have moved the mark.
    let marked = mark.read().await?;
    if mode_on
        && marked == MarkValue::Absent
        && let Some(seq) = entered_last(&mut transaction).await?
    {
        // The mode was turned on for this database and only the mark did not follow. No
        // security event came after it, so there is nothing to suspend, close or mark again.
        transaction.rollback().await?;
        mark.advance(seq)
            .await
            .map_err(|cause| RestoreError::MarkNotAdvanced { seq, cause })?;
        return Ok(EnterOutcome::AlreadyHandled);
    }
    let still_holds = match evidence {
        Evidence::BehindMark { .. } | Evidence::MarkAbsent => matches!(
            standing(&mut transaction, marked).await?,
            Standing::Restored(_)
        ),
        Evidence::SealBeyondDatabase { device } => {
            let already_marked: Option<bool> =
                sqlx::query_scalar("SELECT marked_by_restoration FROM device WHERE id = $1")
                    .bind(device)
                    .fetch_optional(&mut *transaction)
                    .await?;
            !already_marked.ok_or(RestoreError::NoSuchDevice)?
        }
        // With the mode on, an unknown device is listed and nothing more (C11). With it off,
        // the listing is proof for as long as no restored mode handled it.
        Evidence::UnknownDeviceSeal { unknown_device } => {
            let pending: Option<bool> =
                sqlx::query_scalar("SELECT proof_pending FROM unknown_device WHERE id = $1")
                    .bind(unknown_device)
                    .fetch_optional(&mut *transaction)
                    .await?;
            !mode_on && pending == Some(true)
        }
    };
    if !still_holds {
        return Ok(EnterOutcome::AlreadyHandled);
    }

    // The order of every write that changes devices: the Hermes rows first.
    sqlx::query("SELECT id FROM hermes ORDER BY id FOR UPDATE")
        .execute(&mut *transaction)
        .await?;
    let copy_of: Option<i64> = sqlx::query_scalar("SELECT at FROM audit ORDER BY seq DESC LIMIT 1")
        .fetch_optional(&mut *transaction)
        .await?;
    if let MarkValue::At(mark_number) = marked {
        // The next event gets a number above the mark, so the database is ahead of it again.
        // The sequence only ever moves forward: when it already gave a number at or above
        // the mark it is left alone, so a writer that takes a number at this same moment
        // never gets one that is given again.
        sqlx::query(
            "SELECT setval('audit_seq', $1) \
             WHERE $1 > (SELECT CASE WHEN is_called THEN last_value ELSE last_value - 1 END \
                         FROM audit_seq)",
        )
        .bind(mark_number)
        .execute(&mut *transaction)
        .await?;
    }
    // The digest goes with the invite: nothing stays that could be presented again.
    let invites_cancelled = sqlx::query(
        "UPDATE invite SET state = 'cancelled', digest = NULL, join_digest = NULL \
         WHERE state = 'issued'",
    )
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    // The page of a used invite goes too: its request is closed just below.
    sqlx::query("UPDATE invite SET join_digest = NULL WHERE join_digest IS NOT NULL")
        .execute(&mut *transaction)
        .await?;
    let requests_closed = sqlx::query(
        "UPDATE device \
         SET state_reason = CASE state WHEN 'pending' THEN 'restored_pending' \
                                       ELSE 'restored_approved' END, \
             state = 'expired', state_since = $1, slot = NULL, \
             verification_code = NULL, confirm_until = NULL, approved_until = NULL \
         WHERE state IN ('pending', 'approved')",
    )
    .bind(now)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    let suspended = sqlx::query(
        "UPDATE device SET state = 'suspended', state_reason = 'restored', state_since = $1 \
         WHERE state = 'active'",
    )
    .bind(now)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    let marked_now = sqlx::query(
        "UPDATE device SET marked_by_restoration = TRUE WHERE NOT marked_by_restoration",
    )
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    // Every unknown device listed so far is of the restoration that is handled now.
    sqlx::query("UPDATE unknown_device SET proof_pending = FALSE WHERE proof_pending")
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "UPDATE installation SET restored_mode = TRUE, restored_at = $1, restored_copy_of = $2",
    )
    .bind(now)
    .bind(copy_of)
    .execute(&mut *transaction)
    .await?;

    let count = |rows: u64| i64::try_from(rows).unwrap_or(i64::MAX);
    let mut details = Details::new()
        .word("evidence", evidence.word())
        .flag("mode_was_on", mode_on)
        .number("suspended", count(suspended))
        .number("requests_closed", count(requests_closed))
        .number("invites_cancelled", count(invites_cancelled))
        .number("marked", count(marked_now));
    if let Evidence::BehindMark { mark, database } = evidence {
        details = details.number("mark", mark).number("database", database);
    }
    let mut event = Event::new(Action::RestoredModeEntered, Actor::System).with(details);
    match evidence {
        Evidence::SealBeyondDatabase { device } => {
            event = event.about(TargetKind::Device, device);
        }
        Evidence::UnknownDeviceSeal { unknown_device } => {
            event = event.about(TargetKind::UnknownDevice, unknown_device);
        }
        Evidence::BehindMark { .. } | Evidence::MarkAbsent => {}
    }
    let seq = audit::insert(&mut *transaction, now, &event).await?;
    transaction.commit().await?;

    mark.advance(seq)
        .await
        .map_err(|cause| RestoreError::MarkNotAdvanced { seq, cause })?;
    Ok(EnterOutcome::Entered(Entered {
        seq,
        suspended,
        requests_closed,
        invites_cancelled,
        marked: marked_now,
    }))
}

/// The number of the last security event of the database, when that event is the one that
/// turned the restored mode on.
async fn entered_last(connection: &mut PgConnection) -> Result<Option<i64>, sqlx::Error> {
    let last: Option<(i64, String)> =
        sqlx::query_as("SELECT seq, action FROM audit WHERE security ORDER BY seq DESC LIMIT 1")
            .fetch_optional(connection)
            .await?;
    Ok(last
        .filter(|(_, action)| action == Action::RestoredModeEntered.as_str())
        .map(|(seq, _)| seq))
}

/// Ends the restored mode when no suspended device is left. Returns whether it ended now.
///
/// It is a security write: it goes through the check against the mark like any other.
///
/// # Errors
/// [`RestoreError::Audit`] when the security write refuses (a database restored again, for
/// one) or fails.
pub async fn end_when_done(pool: &PgPool, mark: &Mark, now: i64) -> Result<bool, RestoreError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let mode_on: Option<bool> =
        sqlx::query_scalar("SELECT restored_mode FROM installation FOR UPDATE")
            .fetch_optional(write.connection())
            .await?;
    // A suspended device always has its place: said here so that the index of open devices
    // answers.
    let any_suspended: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM device WHERE state = 'suspended' AND slot IS NOT NULL)",
    )
    .fetch_one(write.connection())
    .await?;
    if mode_on != Some(true) || any_suspended {
        write.rollback().await?;
        return Ok(false);
    }
    sqlx::query(
        "UPDATE installation \
         SET restored_mode = FALSE, restored_at = NULL, restored_copy_of = NULL",
    )
    .execute(write.connection())
    .await?;
    write
        .record(now, &Event::new(Action::RestoredModeEnded, Actor::System))
        .await?;
    write.commit().await?;
    Ok(true)
}

/// An entry of the list of unknown devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownDevice {
    /// The row.
    pub id: Uuid,
    /// Whether the identity entered the list now.
    pub newly_listed: bool,
    /// Whether this listing is proof of a restoration that no restored mode handled yet. It
    /// stays so, presentation after presentation, until [`enter`] runs: a hub that stops
    /// between the listing and the mode does not lose the proof.
    pub proof_pending: bool,
}

/// Why an identity enters the list of unknown devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListedAs {
    /// An identity the database does not know: proof of a restoration, unless the restored
    /// mode is already on.
    Stranger,
    /// A request the restored mode closed: the hub knows where it comes from, and it is
    /// never proof again.
    ClosedRequest,
}

/// One presentation to be listed: who, from where, and why.
#[derive(Debug, Clone, Copy)]
pub struct Listing<'a> {
    /// The identity that presented a valid seal.
    pub identity: &'a PublicKey,
    /// The version of its connector, from `hello`. Not verified.
    pub connector_version: Option<&'a DeclaredText>,
    /// The address the connection came from.
    pub address: Option<IpAddr>,
    /// Why it is listed.
    pub listed_as: ListedAs,
}

/// How many times a listing looks again for its entry. The purge may delete an entry past its
/// retention between the insert that met it and the read that follows; the entry written then
/// has the time of now, within its retention, so a second look always settles it. The bound
/// only keeps the loop finite.
const LISTING_LOOKS: u32 = 3;

/// Lists an identity that presented a valid seal and that the database does not know, or only
/// knows as a request a restoration closed: the time, the address and what it declares. An
/// identity is listed once; the first entry stays until the purge deletes it
/// ([`crate::repo::purge`]), and an identity whose entry the purge deleted is listed anew.
///
/// Call it only after a seal opened under the key of `organization`. An attempt with no
/// credential, or with one that does not open, records nothing.
///
/// # Errors
/// [`RestoreError::Database`] when the database refuses.
pub async fn list_unknown_device(
    pool: &PgPool,
    organization: ResolvedOrg,
    listing: &Listing<'_>,
    now: i64,
) -> Result<UnknownDevice, RestoreError> {
    let Listing {
        identity,
        connector_version,
        address,
        listed_as,
    } = *listing;
    let declared = match connector_version {
        Some(version) => serde_json::json!({ "connector_version": version.as_str() }),
        None => serde_json::json!({}),
    };
    let mut transaction = pool.begin().await?;
    // Shared among listings, and exclusive with turning the mode on or off: a listing never
    // stays as pending proof of a restoration the mode began to handle meanwhile.
    let mode_on: Option<bool> =
        sqlx::query_scalar("SELECT restored_mode FROM installation FOR SHARE")
            .fetch_optional(&mut *transaction)
            .await?;
    let proof = listed_as == ListedAs::Stranger && mode_on != Some(true);
    let mut looks = 0u32;
    let device = loop {
        let listed: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO unknown_device (id, organization_id, public_key, declared, address, \
                                         seen_at, proof_pending) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (organization_id, public_key) DO NOTHING \
             RETURNING id",
        )
        .bind(Uuid::new_v4())
        .bind(organization.id())
        .bind(identity.as_bytes().as_slice())
        .bind(sqlx::types::Json(declared.clone()))
        .bind(address.map(|address| address.to_string()))
        .bind(now)
        .bind(proof)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some(id) = listed {
            let mut event = Event::new(Action::UnknownDeviceListed, Actor::System)
                .in_organization(organization)
                .about(TargetKind::UnknownDevice, id);
            if let Some(address) = address {
                event = event.from_address(address);
            }
            audit::record(&mut *transaction, now, &event).await?;
            break UnknownDevice {
                id,
                newly_listed: true,
                proof_pending: proof,
            };
        }
        let found: Option<(Uuid, bool)> = sqlx::query_as(
            "SELECT id, proof_pending FROM unknown_device \
             WHERE organization_id = $1 AND public_key = $2",
        )
        .bind(organization.id())
        .bind(identity.as_bytes().as_slice())
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some((id, proof_pending)) = found {
            break UnknownDevice {
                id,
                newly_listed: false,
                proof_pending,
            };
        }
        // The purge deleted the entry the insert met: the identity is listed anew, as if the
        // purge had come a moment earlier.
        looks = looks.saturating_add(1);
        if looks >= LISTING_LOOKS {
            return Err(sqlx::Error::RowNotFound.into());
        }
    };
    transaction.commit().await?;
    Ok(device)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_evidence_says_what_was_seen_and_shows_no_identifier() {
        let device = Uuid::from_u128(0xfeed);
        let cases = [
            (
                Evidence::BehindMark {
                    mark: 40,
                    database: 12,
                },
                "behind_mark",
            ),
            (Evidence::MarkAbsent, "mark_absent"),
            (
                Evidence::SealBeyondDatabase { device },
                "seal_beyond_database",
            ),
            (
                Evidence::UnknownDeviceSeal {
                    unknown_device: device,
                },
                "unknown_device_seal",
            ),
        ];
        for (evidence, word) in cases {
            assert_eq!(evidence.word(), word);
            let text = evidence.to_string();
            assert!(!text.is_empty());
            assert!(!text.contains("feed"), "{text}");
        }
        let text = Evidence::BehindMark {
            mark: 40,
            database: 12,
        }
        .to_string();
        assert!(text.contains("40") && text.contains("12"), "{text}");
    }
}
