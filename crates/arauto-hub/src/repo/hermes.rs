//! The lines of Hermes (`docs/06-modelo-de-dados.md`, section 3).
//!
//! A name has the restricted format, is not one of the reserved ones, never changes and is
//! never reused: the row of a removed Hermes stays, with the date, and keeps the name taken.

use arauto_protocol::names::HermesName;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::closing::{Closed, close_everything};
use super::{Admin, RepoError, lock_hermes_by_name, notify_hub};
use crate::audit::{self, Action, Details, Event, TargetKind};
use crate::core::model::{Device, Hermes, Slot};
use crate::db::ResolvedOrg;
use crate::mark::Mark;

/// The outcome of [`create`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateOutcome {
    /// The line was created.
    Created(Hermes),
    /// The name is taken by a line that exists.
    NameTaken,
    /// The name belonged to a line that was removed. A name is never reused.
    NameRetired,
}

/// Creates a Hermes line. It is born with no device, no access and no link.
///
/// The format and the reserved names are already checked by the type of `name`.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn create(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    name: &HermesName,
    now: i64,
) -> Result<CreateOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let id = Uuid::new_v4();
    let created: Option<Hermes> = sqlx::query_as(
        "INSERT INTO hermes (id, organization_id, name, created_at) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (organization_id, name) DO NOTHING \
         RETURNING *",
    )
    .bind(id)
    .bind(organization.id())
    .bind(name.as_str())
    .bind(now)
    .fetch_optional(write.connection())
    .await?;
    let Some(created) = created else {
        let removed: Option<bool> = sqlx::query_scalar(
            "SELECT deleted_at IS NOT NULL FROM hermes WHERE organization_id = $1 AND name = $2",
        )
        .bind(organization.id())
        .bind(name.as_str())
        .fetch_optional(write.connection())
        .await?;
        write.rollback().await?;
        return Ok(if removed == Some(true) {
            CreateOutcome::NameRetired
        } else {
            CreateOutcome::NameTaken
        });
    };
    write
        .record(
            now,
            &Event::new(Action::HermesCreated, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Hermes, id)
                .with(Details::new().name("name", name)),
        )
        .await?;
    write.commit().await?;
    Ok(CreateOutcome::Created(created))
}

/// The Hermes with this name, if it exists and was not removed.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn find(
    pool: &PgPool,
    organization: ResolvedOrg,
    name: &HermesName,
) -> Result<Option<Hermes>, RepoError> {
    Ok(sqlx::query_as(
        "SELECT * FROM hermes WHERE organization_id = $1 AND name = $2 AND deleted_at IS NULL",
    )
    .bind(organization.id())
    .bind(name.as_str())
    .fetch_optional(pool)
    .await?)
}

/// The links of a Hermes with the other ones (`docs/13-conversa-entre-hermes.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeerLinks {
    /// A link from this Hermes to another: it can send.
    pub outbound: bool,
    /// A link from another Hermes to this one: it can receive.
    pub inbound: bool,
}

/// Whether the Hermes `hermes` has links with other Hermes, in each direction.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn peer_links(
    pool: &PgPool,
    organization: ResolvedOrg,
    hermes: Uuid,
) -> Result<PeerLinks, RepoError> {
    let (outbound, inbound): (bool, bool) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM peer_link \
                        WHERE organization_id = $1 AND from_hermes_id = $2), \
                EXISTS (SELECT 1 FROM peer_link \
                        WHERE organization_id = $1 AND to_hermes_id = $2)",
    )
    .bind(organization.id())
    .bind(hermes)
    .fetch_one(pool)
    .await?;
    Ok(PeerLinks { outbound, inbound })
}

/// A Hermes line and what is connected to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesLine {
    /// The line.
    pub hermes: Hermes,
    /// The device in use: active, suspended or quarantined.
    pub in_use: Option<Device>,
    /// The request in progress: pending, approved, or quarantined before the first credential.
    pub request: Option<Device>,
    /// Until when the issued invite of this Hermes is accepted, if there is one.
    pub invite_expires_at: Option<i64>,
}

/// Every Hermes line of `organization`, by name. With `include_removed`, the removed ones
/// too.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn list(
    pool: &PgPool,
    organization: ResolvedOrg,
    include_removed: bool,
) -> Result<Vec<HermesLine>, RepoError> {
    let lines: Vec<Hermes> = sqlx::query_as(
        "SELECT * FROM hermes \
         WHERE organization_id = $1 AND ($2 OR deleted_at IS NULL) \
         ORDER BY name",
    )
    .bind(organization.id())
    .bind(include_removed)
    .fetch_all(pool)
    .await?;
    let mut devices: Vec<Device> =
        sqlx::query_as("SELECT * FROM device WHERE organization_id = $1 AND slot IS NOT NULL")
            .bind(organization.id())
            .fetch_all(pool)
            .await?;
    let invites: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT hermes_id, expires_at FROM invite \
         WHERE organization_id = $1 AND state = 'issued'",
    )
    .bind(organization.id())
    .fetch_all(pool)
    .await?;
    Ok(lines
        .into_iter()
        .map(|hermes| {
            let mut take = |slot: Slot| {
                devices
                    .iter()
                    .position(|d| d.hermes_id == hermes.id && d.slot == Some(slot))
                    .map(|at| devices.swap_remove(at))
            };
            let in_use = take(Slot::InUse);
            let request = take(Slot::Request);
            let invite_expires_at = invites
                .iter()
                .find(|(of, _)| *of == hermes.id)
                .map(|(_, expires_at)| *expires_at);
            HermesLine {
                hermes,
                in_use,
                request,
                invite_expires_at,
            }
        })
        .collect())
}

/// What removing a Hermes did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    /// The line, as it was.
    pub hermes: Hermes,
    /// What was closed with it: devices and the issued invite.
    pub closed: Closed,
    /// How many links between Hermes went with it, in either direction.
    pub links_removed: u64,
}

/// The outcome of [`remove`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveOutcome {
    /// The line was removed.
    Removed(Removed),
    /// There is no Hermes with that name.
    NotFound,
}

/// Removes a Hermes line. The row stays, with the date, and its name is never given to
/// another. In the same write its links with other Hermes go away, and everything of it is
/// closed as by a revocation: a removed line with a device still served would be a Hermes
/// nobody can see.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn remove(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    name: &HermesName,
    now: i64,
) -> Result<RemoveOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let Some(locked) = lock_hermes_by_name(write.connection(), organization, name).await? else {
        write.rollback().await?;
        return Ok(RemoveOutcome::NotFound);
    };
    let closed = close_everything(&mut write, organization, &locked, admin, now).await?;
    let links: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "DELETE FROM peer_link \
         WHERE organization_id = $1 AND (from_hermes_id = $2 OR to_hermes_id = $2) \
         RETURNING from_hermes_id, to_hermes_id",
    )
    .bind(organization.id())
    .bind(locked.hermes.id)
    .fetch_all(write.connection())
    .await?;
    for (from, to) in &links {
        write
            .record(
                now,
                &Event::new(Action::PeerLinkRemoved, admin.actor())
                    .in_organization(organization)
                    .about(TargetKind::Hermes, *from)
                    .with(
                        Details::new()
                            .id("to", *to)
                            .word("reason", "hermes_removed"),
                    ),
            )
            .await?;
    }
    sqlx::query(
        "UPDATE hermes SET deleted_at = $3, can_be_woken = FALSE, wake_reason = NULL \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(locked.hermes.id)
    .bind(now)
    .execute(write.connection())
    .await?;
    write
        .record(
            now,
            &Event::new(Action::HermesRemoved, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Hermes, locked.hermes.id)
                .with(Details::new().name("name", &locked.hermes.name)),
        )
        .await?;
    notify_hub(write.connection(), organization, "hermes", locked.hermes.id).await?;
    write.commit().await?;
    Ok(RemoveOutcome::Removed(Removed {
        hermes: locked.hermes,
        closed,
        links_removed: u64::try_from(links.len()).unwrap_or(u64::MAX),
    }))
}

/// The outcome of [`set_wake`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeOutcome {
    /// The mark changed.
    Changed,
    /// It already was as asked. Nothing was written.
    Unchanged,
    /// There is no Hermes with that name.
    NotFound,
}

/// Marks whether a Hermes can be woken: whether a notice may start a run of it.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn set_wake(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    name: &HermesName,
    can_be_woken: bool,
    now: i64,
) -> Result<WakeOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let Some(locked) = lock_hermes_by_name(write.connection(), organization, name).await? else {
        write.rollback().await?;
        return Ok(WakeOutcome::NotFound);
    };
    if locked.hermes.can_be_woken == can_be_woken {
        write.rollback().await?;
        return Ok(WakeOutcome::Unchanged);
    }
    sqlx::query(
        "UPDATE hermes SET can_be_woken = $3, wake_reason = NULL \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(locked.hermes.id)
    .bind(can_be_woken)
    .execute(write.connection())
    .await?;
    write
        .record(
            now,
            &Event::new(Action::HermesWakeChanged, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Hermes, locked.hermes.id)
                .with(Details::new().flag("can_be_woken", can_be_woken)),
        )
        .await?;
    notify_hub(write.connection(), organization, "hermes", locked.hermes.id).await?;
    write.commit().await?;
    Ok(WakeOutcome::Changed)
}
