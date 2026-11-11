//! The record of channel connections (`docs/06-modelo-de-dados.md`, `conexao_canal`).
//!
//! One row per connection of a known device that answered its check. The row says when a
//! credential was issued on it and when the swap was confirmed. A connection that received a
//! credential and ended without the "saved" stays on record, and three of them in 24 hours,
//! from the same device, raise an alert, even with confirmed connections in between: it may
//! be a bad network, it may be someone trying a copy (C7).
//!
//! The endings of the connections of one device run one after the other, behind the lock of
//! its Hermes: the count of the swaps left unconfirmed is only right when each ending sees
//! the ones before it. T1 and T2 lock the row of their connection under that same lock, and
//! refuse a connection that is not on record as open.

use std::net::IpAddr;

use arauto_protocol::limits::DAY_MS;
use arauto_protocol::message::DeclaredText;
use sqlx::postgres::{PgConnection, PgPool};
use uuid::Uuid;

use super::alert::{self, AlertDetails, AlertKind, NewAlert};
use super::{RepoError, lock_hermes_of_device};
use crate::audit::{self, Action, Actor, Details, Event, TargetKind};
use crate::db::ResolvedOrg;

/// How many unconfirmed swaps of one device, in 24 hours, raise the alert.
pub const UNCONFIRMED_SWAPS_ALERT: i64 = 3;

/// A channel connection that is on record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelConnection {
    /// The row.
    pub id: Uuid,
    /// The device that proved its identity on it.
    pub device: Uuid,
    /// The address it came from.
    pub address: Option<IpAddr>,
    /// The version the connector declared in `hello`. Not verified.
    pub connector_version: DeclaredText,
    /// The protocol version that was agreed. Zero for an outdated connector.
    pub proto: u32,
}

/// Records a connection of `device` that answered its check.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses; [`RepoError::Inconsistent`] when
/// `device` is not a device of `organization`: nothing is recorded.
pub async fn open(
    pool: &PgPool,
    organization: ResolvedOrg,
    device: Uuid,
    address: Option<IpAddr>,
    connector_version: &DeclaredText,
    proto: u32,
    now: i64,
) -> Result<ChannelConnection, RepoError> {
    let id = Uuid::new_v4();
    let recorded = sqlx::query(
        "INSERT INTO channel_connection (id, organization_id, device_id, address, \
                                         connector_version, opened_at) \
         SELECT $1, $2, d.id, $4, $5, $6 FROM device d \
         WHERE d.organization_id = $2 AND d.id = $3",
    )
    .bind(id)
    .bind(organization.id())
    .bind(device)
    .bind(address.map(|address| address.to_string()))
    .bind(connector_version.as_str())
    .bind(now)
    .execute(pool)
    .await?
    .rows_affected();
    if recorded == 0 {
        // Nothing was written: a connection of a device that is not of this organization is
        // not on record, and whoever asked for it does not get one to hand to T1.
        return Err(RepoError::Inconsistent(
            "the device of a connection is not in its organization",
        ));
    }
    Ok(ChannelConnection {
        id,
        device,
        address,
        connector_version: connector_version.clone(),
        proto,
    })
}

/// Why a connection ended. The catalog is closed; the word goes to `close_reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloseReason {
    /// The other side closed, or the network dropped.
    Dropped,
    /// Ninety seconds with nothing received.
    Silence,
    /// A protocol error, a limit, or a deadline before the session (AR-204).
    Protocol,
    /// A newer attempt of the same device took its place before it became a channel (AR-206).
    ReplacedAttempt,
    /// It did not answer the probe and another connection came in (AR-208).
    GivenUpForDead,
    /// The "saved" did not arrive in time.
    SavedTimeout,
    /// The "saved" was not about the pending credential of this connection.
    SavedRefused,
    /// The device is no longer approved or active: its code was answered.
    DeviceEnded,
    /// The presentation without swap ended, as it always does.
    Presented,
    /// The hub is stopping (AR-205).
    HubStopping,
    /// The hub found the row open when it started: it had stopped without closing it.
    HubStopped,
}

impl CloseReason {
    /// The word stored in `channel_connection.close_reason`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            CloseReason::Dropped => "dropped",
            CloseReason::Silence => "silence",
            CloseReason::Protocol => "protocol",
            CloseReason::ReplacedAttempt => "replaced_attempt",
            CloseReason::GivenUpForDead => "given_up_for_dead",
            CloseReason::SavedTimeout => "saved_timeout",
            CloseReason::SavedRefused => "saved_refused",
            CloseReason::DeviceEnded => "device_ended",
            CloseReason::Presented => "presented",
            CloseReason::HubStopping => "hub_stopping",
            CloseReason::HubStopped => "hub_stopped",
        }
    }
}

/// What closing a connection found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClosedConnection {
    /// The connection had received a credential and ended without its "saved".
    pub unconfirmed: bool,
    /// That was at least the third in 24 hours, and the alert was raised now.
    pub alerted: bool,
}

/// What every unconfirmed row has: a credential issued and no confirmation after it.
const UNCONFIRMED: &str = "credential_issued_at IS NOT NULL \
     AND (swap_confirmed_at IS NULL OR swap_confirmed_at < credential_issued_at)";

/// Records one connection that ended without its "saved", and raises the alert at the third
/// of the device in 24 hours. One alert per device per 24 hours.
///
/// Called under the lock of the Hermes of the device: the count and the question "was the
/// alert already raised?" are only right when two endings of the same device do not run at
/// the same moment.
async fn note_unconfirmed(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    device: Uuid,
    address: Option<IpAddr>,
    now: i64,
) -> Result<bool, RepoError> {
    let since = now.saturating_sub(DAY_MS);
    let mut event = Event::new(Action::CredentialUnconfirmed, Actor::Device { id: device })
        .in_organization(organization)
        .about(TargetKind::Device, device);
    if let Some(address) = address {
        event = event.from_address(address);
    }
    audit::record(&mut *connection, now, &event).await?;
    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM channel_connection \
         WHERE organization_id = $1 AND device_id = $2 AND closed_at > $3 AND {UNCONFIRMED}"
    )))
    .bind(organization.id())
    .bind(device)
    .bind(since)
    .fetch_one(&mut *connection)
    .await?;
    if count < UNCONFIRMED_SWAPS_ALERT
        || alert::raised_since(
            &mut *connection,
            organization,
            AlertKind::UnconfirmedSwaps,
            device,
            since,
        )
        .await?
    {
        return Ok(false);
    }
    let hermes: Option<Uuid> =
        sqlx::query_scalar("SELECT hermes_id FROM device WHERE organization_id = $1 AND id = $2")
            .bind(organization.id())
            .bind(device)
            .fetch_optional(&mut *connection)
            .await?;
    alert::raise(
        connection,
        organization,
        now,
        &NewAlert {
            kind: AlertKind::UnconfirmedSwaps,
            hermes,
            device: Some(device),
            details: AlertDetails::new()
                .number("count", count)
                .address("from", address),
        },
    )
    .await?;
    Ok(true)
}

/// Records the end of a connection. A connection that is already closed is left as it is.
///
/// The endings of the connections of one device queue up behind the lock of its Hermes, the
/// first lock of every write about a device: several can end at the same moment (the hub
/// stopping, a double connection), and each one has to see the ones before it to count the
/// swaps left unconfirmed.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn close(
    pool: &PgPool,
    organization: ResolvedOrg,
    connection: &ChannelConnection,
    reason: CloseReason,
    now: i64,
) -> Result<ClosedConnection, RepoError> {
    let mut transaction = pool.begin().await?;
    if lock_hermes_of_device(&mut transaction, organization, connection.device)
        .await?
        .is_none()
    {
        // Not a device of this organization: it has no row here to close.
        return Ok(ClosedConnection::default());
    }
    let closed: Option<(bool,)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE channel_connection SET closed_at = $3, close_reason = $4 \
         WHERE organization_id = $1 AND id = $2 AND closed_at IS NULL \
         RETURNING ({UNCONFIRMED})"
    )))
    .bind(organization.id())
    .bind(connection.id)
    .bind(now)
    .bind(reason.as_str())
    .fetch_optional(&mut *transaction)
    .await?;
    let mut outcome = ClosedConnection::default();
    if closed == Some((true,)) {
        outcome.unconfirmed = true;
        outcome.alerted = note_unconfirmed(
            &mut transaction,
            organization,
            connection.device,
            connection.address,
            now,
        )
        .await?;
    }
    transaction.commit().await?;
    Ok(outcome)
}

/// Closes the rows a hub that stopped without warning left open. The hub calls it once, when
/// it starts, before accepting connections. Returns how many rows it closed.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn close_left_open(
    pool: &PgPool,
    organization: ResolvedOrg,
    now: i64,
) -> Result<u64, RepoError> {
    let devices: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT device_id FROM channel_connection \
         WHERE organization_id = $1 AND closed_at IS NULL \
         ORDER BY device_id",
    )
    .bind(organization.id())
    .fetch_all(pool)
    .await?;
    let mut total = 0u64;
    // One device at a time, each under the lock of its Hermes, like any other ending.
    for device in devices {
        let mut transaction = pool.begin().await?;
        if lock_hermes_of_device(&mut transaction, organization, device)
            .await?
            .is_none()
        {
            continue;
        }
        let closed: Vec<(bool,)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "UPDATE channel_connection SET closed_at = $3, close_reason = $4 \
             WHERE organization_id = $1 AND device_id = $2 AND closed_at IS NULL \
             RETURNING ({UNCONFIRMED})"
        )))
        .bind(organization.id())
        .bind(device)
        .bind(now)
        .bind(CloseReason::HubStopped.as_str())
        .fetch_all(&mut *transaction)
        .await?;
        for (unconfirmed,) in &closed {
            if *unconfirmed {
                note_unconfirmed(&mut transaction, organization, device, None, now).await?;
            }
        }
        transaction.commit().await?;
        total = total.saturating_add(u64::try_from(closed.len()).unwrap_or(u64::MAX));
    }
    Ok(total)
}

/// A session that was being served did not answer the probe and was closed for another
/// connection of the same device: "the connection of {name} was taken by another, coming
/// from {address}" (C9). Raises the alert, every time.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn taken(
    pool: &PgPool,
    organization: ResolvedOrg,
    device: Uuid,
    taken_from: Option<IpAddr>,
    now: i64,
) -> Result<(), RepoError> {
    let mut transaction = pool.begin().await?;
    let hermes: Option<Uuid> =
        sqlx::query_scalar("SELECT hermes_id FROM device WHERE organization_id = $1 AND id = $2")
            .bind(organization.id())
            .bind(device)
            .fetch_optional(&mut *transaction)
            .await?;
    let Some(hermes) = hermes else {
        return Ok(());
    };
    alert::raise(
        &mut transaction,
        organization,
        now,
        &NewAlert {
            kind: AlertKind::ConnectionTaken,
            hermes: Some(hermes),
            device: Some(device),
            details: AlertDetails::new().address("from", taken_from),
        },
    )
    .await?;
    let mut event = Event::new(Action::ConnectionTaken, Actor::System)
        .in_organization(organization)
        .about(TargetKind::Device, device)
        .with(Details::new().id("hermes", hermes));
    if let Some(address) = taken_from {
        event = event.from_address(address);
    }
    audit::record(&mut *transaction, now, &event).await?;
    transaction.commit().await?;
    Ok(())
}

/// How the row of a connection stands, read under its lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OnRecord {
    /// There is no such connection of this device in this organization, or it was closed.
    Closed,
    /// The connection is open.
    Open {
        /// A swap was confirmed on it: the session of this connection began.
        swap_confirmed: bool,
    },
}

/// Locks the row of a connection and says how it stands. Called under the lock of the Hermes
/// of the device, by T1 and T2: a credential is only issued or promoted on a connection the
/// record shows as open and as being of that device.
pub(crate) async fn lock_on_record(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    of: &ChannelConnection,
) -> Result<OnRecord, sqlx::Error> {
    let row: Option<(Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT swap_confirmed_at, closed_at FROM channel_connection \
         WHERE organization_id = $1 AND id = $2 AND device_id = $3 \
         FOR NO KEY UPDATE",
    )
    .bind(organization.id())
    .bind(of.id)
    .bind(of.device)
    .fetch_optional(connection)
    .await?;
    Ok(match row {
        Some((swap_confirmed_at, None)) => OnRecord::Open {
            swap_confirmed: swap_confirmed_at.is_some(),
        },
        Some((_, Some(_))) | None => OnRecord::Closed,
    })
}

/// T1 issued a credential on this connection. Called inside the transaction of T1.
pub(crate) async fn mark_issued(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    id: Uuid,
    now: i64,
) -> Result<(), sqlx::Error> {
    // Strictly after the last confirmation, even within the same millisecond.
    sqlx::query(
        "UPDATE channel_connection \
         SET credential_issued_at = GREATEST($3, COALESCE(swap_confirmed_at + 1, $3)) \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(id)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

/// T2 (or a promotion in T1) confirmed the swap of this connection. Called inside that
/// transaction.
pub(crate) async fn mark_confirmed(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    id: Uuid,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE channel_connection \
         SET swap_confirmed_at = GREATEST($3, COALESCE(credential_issued_at, $3)) \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(id)
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_reason_has_its_own_plain_word() {
        let all = [
            CloseReason::Dropped,
            CloseReason::Silence,
            CloseReason::Protocol,
            CloseReason::ReplacedAttempt,
            CloseReason::GivenUpForDead,
            CloseReason::SavedTimeout,
            CloseReason::SavedRefused,
            CloseReason::DeviceEnded,
            CloseReason::Presented,
            CloseReason::HubStopping,
            CloseReason::HubStopped,
        ];
        let words: BTreeSet<&str> = all.iter().map(|reason| reason.as_str()).collect();
        assert_eq!(words.len(), all.len());
        for word in words {
            assert!(
                word.bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_'),
                "{word}"
            );
        }
    }
}
