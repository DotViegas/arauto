//! The tasks of the whole hub (`docs/15-mvp.md`, section 2, "Como o hub fica sabendo", and
//! rule 11).
//!
//! - **The sweep**, every 2 seconds, and whenever the listener asks: the audit mark (a database
//!   restored with the hub running turns the restored mode on), the deadlines of each
//!   organization, the state of every device with a channel connection in one query per
//!   organization (what no longer serves gets the `closing` of its code: this, and not the
//!   notice, is what closes a revoked device in less than 5 seconds), the refused enrollments
//!   and the end of the restored mode. It reads through a pool of its own, apart from the one
//!   of the connections: a return in mass that holds every connection of that pool does not
//!   delay a revocation.
//! - **The listener** of the notices of the admin command (`NOTIFY` on `arauto_admin`), on a
//!   connection of its own, outside the pool. A notice names a device or a Hermes, and the
//!   connections of it read their state again at once. The notice is a hint: it is not kept,
//!   so every time the listener connects again it asks for a sweep.
//! - **The purge**, every 10 minutes by default ([`super::settings::PurgeSettings`]): what
//!   passed its retention, by the time of the hub, organization by organization, in small
//!   batches ([`crate::repo::purge`]). A task of its own, so that a long run never delays the
//!   sweep; its batches go through the pool of the connections, one connection at a time (the
//!   pool of the sweep is only the sweep's: `docs/15-mvp.md`, section 2, rule 12). When the
//!   hub stops, the purge stops between two batches, and the stop waits for it.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use arauto_protocol::errors::ErrorCode;
use sqlx::postgres::PgListener;
use uuid::Uuid;

use super::Hub;
use super::registry::{Entry, Place, Status};
use super::settings::duration;
use crate::audit::AuditError;
use crate::core::model::DeviceState;
use crate::db::{self, ResolvedOrg, SystemTask};
use crate::repo::device::Standing;
use crate::repo::{self, ADMIN_CHANNEL, RepoError};
use crate::restore::{self, RestoreError};

/// How long the listener waits before it tries to connect again.
const LISTENER_RETRY: Duration = Duration::from_secs(1);

/// The sweep, until the hub stops.
pub(crate) async fn sweeper(hub: Arc<Hub>) {
    let mut stopping = hub.stopping();
    let every = duration(hub.settings().sweep_interval_ms.max(1));
    loop {
        tokio::select! {
            biased;
            _ = stopping.wait_for(|stopping| *stopping) => return,
            () = hub.sweep_asked() => {}
            () = tokio::time::sleep(every) => {}
        }
        sweep(&hub).await;
    }
}

/// One sweep.
async fn sweep(hub: &Hub) {
    let now = hub.now_ms();
    let pool = hub.sweep_pool();
    match restore::check(pool, hub.mark()).await {
        Ok(restore::Standing::Restored(evidence)) => hub.enter_restored_mode(evidence).await,
        Ok(restore::Standing::Consistent | restore::Standing::NotInstalled) => {}
        Err(error) => tracing::error!(%error, "the sweep could not read the audit mark"),
    }
    match db::organizations_for_system_task(pool, SystemTask::Deadlines).await {
        Ok(organizations) => {
            for organization in organizations {
                match repo::sweep::sweep(pool, hub.mark(), organization, now).await {
                    Ok(_) => {}
                    Err(RepoError::Restored(evidence)) => {
                        hub.enter_restored_mode(evidence).await;
                    }
                    Err(error) => tracing::error!(%error, "the deadlines could not be applied"),
                }
            }
        }
        Err(error) => tracing::error!(%error, "the organizations could not be listed"),
    }
    check_standings(hub).await;
    if let Err(error) = repo::enroll::flush_refused(pool, hub.refusals(), now).await {
        tracing::error!(%error, "the refused enrollments could not be written");
    }
    // Only while the mode is on: ending it locks the row of the installation, which every
    // listing of an unknown device also takes.
    let mode_on = matches!(
        restore::installation(pool).await,
        Ok(Some(installation)) if installation.restored_mode
    );
    if !mode_on {
        return;
    }
    match restore::end_when_done(pool, hub.mark(), now).await {
        Ok(_) => {}
        Err(RestoreError::Audit(AuditError::Restored(evidence))) => {
            hub.enter_restored_mode(evidence).await;
        }
        Err(error) => tracing::error!(%error, "the end of the restored mode could not be written"),
    }
}

/// Reads again the state of every device with a channel connection, one query per
/// organization. A session whose device no longer serves gets the `closing` of its code; a
/// waiting connection whose request was resolved is poked, and tells the connector itself.
pub(crate) async fn check_standings(hub: &Hub) {
    let mut by_organization: HashMap<ResolvedOrg, Vec<(Arc<Entry>, Place)>> = HashMap::new();
    for entry in hub.registry().all() {
        if let Some(place) = entry.place() {
            by_organization
                .entry(place.organization)
                .or_default()
                .push((entry, place));
        }
    }
    for (organization, entries) in by_organization {
        let mut devices: Vec<Uuid> = entries.iter().map(|(_, place)| place.device).collect();
        devices.sort_unstable();
        devices.dedup();
        let standings: HashMap<Uuid, Standing> =
            match repo::device::standings(hub.sweep_pool(), organization, &devices).await {
                Ok(standings) => standings
                    .into_iter()
                    .map(|standing| (standing.device, standing))
                    .collect(),
                Err(error) => {
                    tracing::error!(%error, "the state of the connected devices could not be read");
                    continue;
                }
            };
        for (entry, place) in entries {
            let standing = standings.get(&place.device);
            match entry.status() {
                Status::Session => {
                    if !standing.is_some_and(Standing::may_serve) {
                        let code = standing
                            .and_then(Standing::ended_code)
                            .unwrap_or(ErrorCode::Ar504);
                        entry.handle().close(code);
                    }
                }
                Status::Waiting => {
                    if standing.is_none_or(|standing| standing.state != DeviceState::Pending) {
                        entry.poke();
                    }
                }
                // T1 and T2 read the device again under its lock.
                Status::Attempt => {}
            }
        }
    }
}

/// The purge, until the hub stops: one run each interval, the first one an interval after the
/// start. A run that sees the stop ends between two batches.
pub(crate) async fn purger(hub: Arc<Hub>) {
    let mut stopping = hub.stopping();
    let every = duration(hub.settings().purge.interval_ms.max(1));
    loop {
        tokio::select! {
            biased;
            _ = stopping.wait_for(|stopping| *stopping) => return,
            () = tokio::time::sleep(every) => {}
        }
        purge(&hub).await;
    }
}

/// One run of the purge, at the time of the hub, through every organization.
async fn purge(hub: &Hub) {
    let now = hub.now_ms();
    let pool = hub.pool();
    let retention = match repo::purge::retention(pool).await {
        Ok(retention) => retention,
        Err(error) => {
            tracing::error!(%error, "the purge could not read the retention of the tables");
            return;
        }
    };
    let organizations = match db::organizations_for_system_task(pool, SystemTask::Purge).await {
        Ok(organizations) => organizations,
        Err(error) => {
            tracing::error!(%error, "the purge could not list the organizations");
            return;
        }
    };
    let batch_rows = hub.settings().purge.batch_rows;
    let mut rows = 0u64;
    for organization in organizations {
        let purged = repo::purge::purge(pool, organization, &retention, now, batch_rows, || {
            !hub.is_stopping()
        })
        .await;
        match purged {
            Ok(purged) => {
                rows = rows.saturating_add(purged.rows());
                if purged.stopped {
                    break;
                }
            }
            // What was deleted stays deleted; the next run goes on from there.
            Err(error) => tracing::error!(%error, "a batch of the purge failed"),
        }
    }
    if rows > 0 {
        tracing::info!(rows, "the purge deleted what passed its retention");
    }
}

/// The listener of the notices of the admin command, until the hub stops.
pub(crate) async fn listener(hub: Arc<Hub>) {
    if !hub.settings().listen_admin {
        return;
    }
    let mut stopping = hub.stopping();
    loop {
        let connected = tokio::select! {
            biased;
            _ = stopping.wait_for(|stopping| *stopping) => return,
            connected = connect(&hub) => connected,
        };
        let Some(mut listener) = connected else {
            tokio::select! {
                biased;
                _ = stopping.wait_for(|stopping| *stopping) => return,
                () = tokio::time::sleep(LISTENER_RETRY) => {}
            }
            continue;
        };
        // What the admin command said while nobody listened is lost: sweep now.
        hub.sweep_soon();
        loop {
            let received = tokio::select! {
                biased;
                _ = stopping.wait_for(|stopping| *stopping) => return,
                received = listener.try_recv() => received,
            };
            match received {
                Ok(Some(notification)) => on_notice(&hub, notification.payload()),
                // The connection dropped and came back: notices may have been lost.
                Ok(None) => hub.sweep_soon(),
                Err(_) => break,
            }
        }
    }
}

/// A connection of its own, outside the pool, listening on the channel of the admin command.
async fn connect(hub: &Hub) -> Option<PgListener> {
    let mut listener = match PgListener::connect(hub.database_url().expose()).await {
        Ok(listener) => listener,
        Err(_) => {
            // The address is not part of the message: it may carry a password.
            tracing::warn!("the listener of the admin notices could not connect");
            return None;
        }
    };
    if listener.listen(ADMIN_CHANNEL).await.is_err() {
        tracing::warn!("the listener of the admin notices could not listen");
        return None;
    }
    Some(listener)
}

/// `kind:id`, written by the admin command: the connections of that device, or of every
/// device of that Hermes, read their state again. Anything else makes the sweep run now.
fn on_notice(hub: &Hub, payload: &str) {
    let named = payload
        .split_once(':')
        .and_then(|(_, id)| Uuid::parse_str(id).ok());
    match named {
        Some(id) => {
            for entry in hub.registry().of(id) {
                entry.poke();
            }
        }
        None => hub.sweep_soon(),
    }
}
