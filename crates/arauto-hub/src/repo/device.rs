//! What a connection needs to know about its device, and the key of its organization.
//!
//! These are reads. The hub asks [`standing`] at every call and at every sweep: revoking and
//! every other change of state take effect at the next question, with no memory in between.

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::ChannelState;
use arauto_protocol::names::HermesName;
use arauto_protocol::noise::PublicKey;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::RepoError;
use crate::core::model::{self, Device, DeviceState, StateReason};
use crate::db::ResolvedOrg;
use crate::vault::{OrganizationKey, Vault};

/// The device with this identity, in any state. The row of a device is never erased: a
/// closed identity that comes back is recognized and told its situation.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn by_identity(
    pool: &PgPool,
    organization: ResolvedOrg,
    identity: &PublicKey,
) -> Result<Option<Device>, RepoError> {
    Ok(
        sqlx::query_as("SELECT * FROM device WHERE organization_id = $1 AND public_key = $2")
            .bind(organization.id())
            .bind(identity.as_bytes().as_slice())
            .fetch_optional(pool)
            .await?,
    )
}

/// The device with this identifier.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn by_id(
    pool: &PgPool,
    organization: ResolvedOrg,
    device: Uuid,
) -> Result<Option<Device>, RepoError> {
    Ok(
        sqlx::query_as("SELECT * FROM device WHERE organization_id = $1 AND id = $2")
            .bind(organization.id())
            .bind(device)
            .fetch_optional(pool)
            .await?,
    )
}

/// How a device stands right now: what a session is allowed to go on with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// The device.
    pub device: Uuid,
    /// Its state.
    pub state: DeviceState,
    /// Why it is in that state.
    pub reason: Option<StateReason>,
    /// The Hermes it belongs to.
    pub hermes: Uuid,
    /// The name of the Hermes.
    pub name: HermesName,
    /// Whether the Hermes can be woken.
    pub can_be_woken: bool,
}

impl Standing {
    /// The code a connection of this device gets, or `None` while it is pending, approved or
    /// active.
    #[must_use]
    pub const fn ended_code(&self) -> Option<ErrorCode> {
        model::ended_code(self.state, self.reason)
    }

    /// The `state` message a channel connection of this device gets.
    #[must_use]
    pub const fn channel_state(&self) -> ChannelState {
        model::channel_state(self.state, self.reason)
    }

    /// Whether a session of this device may serve tools and notices: only an active one.
    #[must_use]
    pub fn may_serve(&self) -> bool {
        self.state == DeviceState::Active
    }
}

type StandingRow = (Uuid, String, Option<String>, Uuid, String, bool);

fn standing_of(row: StandingRow) -> Result<Standing, RepoError> {
    let (device, state, reason, hermes, name, can_be_woken) = row;
    Ok(Standing {
        device,
        state: DeviceState::parse(&state)
            .ok_or(RepoError::Inconsistent("a device in an unknown state"))?,
        reason: reason
            .map(|reason| {
                StateReason::parse(&reason)
                    .ok_or(RepoError::Inconsistent("a device with an unknown reason"))
            })
            .transpose()?,
        hermes,
        name: HermesName::parse(&name)
            .map_err(|_| RepoError::Inconsistent("a Hermes with an invalid name"))?,
        can_be_woken,
    })
}

/// How each of these devices stands now, in one query by index. A device that is not of
/// `organization` is left out. The sweep of the hub asks this for every open connection.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses; [`RepoError::Inconsistent`] when a row
/// has a state this program does not know.
pub async fn standings(
    pool: &PgPool,
    organization: ResolvedOrg,
    devices: &[Uuid],
) -> Result<Vec<Standing>, RepoError> {
    if devices.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<StandingRow> = sqlx::query_as(
        "SELECT d.id, d.state, d.state_reason, h.id, h.name, h.can_be_woken \
         FROM device d \
         JOIN hermes h ON h.organization_id = d.organization_id AND h.id = d.hermes_id \
         WHERE d.organization_id = $1 AND d.id = ANY($2)",
    )
    .bind(organization.id())
    .bind(devices)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(standing_of).collect()
}

/// How one device stands now. `None` when it is not of `organization`.
///
/// # Errors
/// As [`standings`].
pub async fn standing(
    pool: &PgPool,
    organization: ResolvedOrg,
    device: Uuid,
) -> Result<Option<Standing>, RepoError> {
    Ok(standings(pool, organization, &[device]).await?.pop())
}

/// An identity of the list of unknown devices, as the admin reads it (C11): a Hermes that was
/// connected, or reconnected, after the copy the hub came back from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownListed {
    /// The row.
    pub id: Uuid,
    /// When it presented its valid seal for the first time.
    pub seen_at: i64,
    /// The address it came from, as stored. Show it escaped.
    pub address: Option<String>,
    /// The version its connector declared. Not verified: show it escaped.
    pub connector_version: Option<String>,
    /// Whether it is proof of a restoration that no restored mode handled yet.
    pub proof_pending: bool,
}

type UnknownRow = (Uuid, i64, Option<String>, Option<String>, bool);

/// The unknown devices listed in `organization`, newest first, at most `limit`.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn unknown_listed(
    pool: &PgPool,
    organization: ResolvedOrg,
    limit: u32,
) -> Result<Vec<UnknownListed>, RepoError> {
    let rows: Vec<UnknownRow> = sqlx::query_as(
        "SELECT id, seen_at, address, declared->>'connector_version', proof_pending \
         FROM unknown_device \
         WHERE organization_id = $1 \
         ORDER BY seen_at DESC, id \
         LIMIT $2",
    )
    .bind(organization.id())
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, seen_at, address, connector_version, proof_pending)| UnknownListed {
                id,
                seen_at,
                address,
                connector_version,
                proof_pending,
            },
        )
        .collect())
}

/// The keys of `organization`, opened with the master key: what seals and opens the
/// credentials of its devices. The hub keeps the result for as long as it runs.
///
/// # Errors
/// [`RepoError::Vault`] when the stored key does not open with this master key;
/// [`RepoError::Inconsistent`] when the organization has no row.
pub async fn organization_key(
    pool: &PgPool,
    organization: ResolvedOrg,
    vault: &Vault,
) -> Result<OrganizationKey, RepoError> {
    let key_enc: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT key_enc FROM organization WHERE id = $1")
            .bind(organization.id())
            .fetch_optional(pool)
            .await?;
    let key_enc = key_enc.ok_or(RepoError::Inconsistent(
        "a resolved organization has no row",
    ))?;
    Ok(vault.open_organization_key(organization.id(), &key_enc)?)
}
