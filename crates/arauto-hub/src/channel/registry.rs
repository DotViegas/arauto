//! The record, in memory, of the channel connections of each device
//! (`docs/14-protocolo-do-canal.md`, section 6; `docs/15-mvp.md`, section 3).
//!
//! A map split by identity: each identity holds its channel connections, each with the order
//! number it got at the end of the handshake. What the steps of the swap ask of it:
//!
//! | Step | Question |
//! |---|---|
//! | 5 | Is this the newest connection of the identity? |
//! | 6 | Which older ones are not established yet? They close with AR-206. |
//! | 8 | Which ones are established (waiting or in session)? Each gets a `check`, and the one that answers proves two holders of the same files. |
//!
//! Each identity also has its **turn to be judged**: from the question of step 5 to the end of
//! T1, one connection of the identity at a time. Without it, an attempt that passed step 5
//! and was then abandoned could run its T1 after a newer attempt already swapped, and judge a
//! credential that died meanwhile: the legitimate device would go to quarantine.
//!
//! The sweep reads it too: every connection whose device is known, by organization, to read
//! their states again in one query per organization. A second hub serving the same
//! organization is not handled in the MVP: this record lives in one process.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::CheckNonce;
use dashmap::DashMap;
use tokio::sync::{Notify, OwnedMutexGuard, oneshot};
use uuid::Uuid;

use super::link::{Link, LinkHandle};
use crate::db::ResolvedOrg;

/// Where a connection is in its conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    /// Before it is established: the presentation, the check, the swap.
    Attempt,
    /// Established: a pending device waiting for the admin.
    Waiting,
    /// Established: the session of the channel, serving.
    Session,
}

impl Status {
    /// Waiting and in session are the established connections.
    pub(crate) const fn is_established(self) -> bool {
        matches!(self, Status::Waiting | Status::Session)
    }
}

/// The device a connection is of, once the hub found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Place {
    /// Its organization.
    pub(crate) organization: ResolvedOrg,
    /// The device.
    pub(crate) device: Uuid,
    /// Its Hermes.
    pub(crate) hermes: Uuid,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Plain values: a panic elsewhere leaves them usable.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One channel connection.
pub(crate) struct Entry {
    serial: u64,
    identity: [u8; 32],
    handle: LinkHandle,
    status: Mutex<Status>,
    place: Mutex<Option<Place>>,
    poke: Notify,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("serial", &self.serial)
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl Entry {
    /// The order number of the connection.
    pub(crate) const fn serial(&self) -> u64 {
        self.serial
    }

    /// The connection itself.
    pub(crate) const fn handle(&self) -> &LinkHandle {
        &self.handle
    }

    pub(crate) fn status(&self) -> Status {
        *lock(&self.status)
    }

    pub(crate) fn set_status(&self, status: Status) {
        *lock(&self.status) = status;
    }

    pub(crate) fn place(&self) -> Option<Place> {
        *lock(&self.place)
    }

    pub(crate) fn set_place(&self, place: Place) {
        *lock(&self.place) = Some(place);
    }

    /// Asks the task of the connection to read the state of its device again.
    pub(crate) fn poke(&self) {
        self.poke.notify_one();
    }

    /// Resolves when the connection was poked. A poke that came while nobody waited is kept.
    pub(crate) async fn poked(&self) {
        self.poke.notified().await;
    }

    /// Sends `check` to this connection and returns what resolves when it answers. `None`
    /// when the connection is already closed. The reader of the connection answers it, so a
    /// session busy with the database is never given up for dead because of that
    /// ([`LinkHandle::probe`]).
    pub(crate) fn probe(&self, nonce: CheckNonce) -> Option<oneshot::Receiver<()>> {
        self.handle.probe(nonce)
    }
}

/// The connections of one identity, and its turn to be judged.
#[derive(Debug, Default)]
struct Identity {
    entries: Vec<Arc<Entry>>,
    turn: Arc<tokio::sync::Mutex<()>>,
}

/// The record of every channel connection of the hub.
#[derive(Debug, Default)]
pub(crate) struct Registry {
    by_identity: DashMap<[u8; 32], Identity>,
}

impl Registry {
    /// Records a channel connection. Dropped, the returned guard takes it out.
    pub(crate) fn register(self: &Arc<Self>, link: &Link) -> Registration {
        let info = link.info();
        let entry = Arc::new(Entry {
            serial: info.serial,
            identity: *info.identity.as_bytes(),
            handle: link.handle(),
            status: Mutex::new(Status::Attempt),
            place: Mutex::new(None),
            poke: Notify::new(),
        });
        let turn = {
            let mut identity = self.by_identity.entry(entry.identity).or_default();
            identity.entries.push(Arc::clone(&entry));
            Arc::clone(&identity.turn)
        };
        Registration {
            registry: Arc::clone(self),
            entry,
            turn,
        }
    }

    fn remove(&self, entry: &Entry) {
        self.by_identity
            .remove_if_mut(&entry.identity, |_, identity| {
                identity
                    .entries
                    .retain(|other| other.serial != entry.serial);
                // Whoever holds the turn is a connection of the record: with none left, nobody
                // holds it.
                identity.entries.is_empty()
            });
    }

    /// The channel connections of one identity.
    pub(crate) fn of_identity(&self, identity: &[u8; 32]) -> Vec<Arc<Entry>> {
        self.by_identity
            .get(identity)
            .map(|identity| identity.entries.clone())
            .unwrap_or_default()
    }

    /// Every channel connection.
    pub(crate) fn all(&self) -> Vec<Arc<Entry>> {
        self.by_identity
            .iter()
            .flat_map(|identity| identity.value().entries.clone())
            .collect()
    }

    /// The channel connections of a device, or of every device of a Hermes.
    pub(crate) fn of(&self, id: Uuid) -> Vec<Arc<Entry>> {
        self.all()
            .into_iter()
            .filter(|entry| {
                entry
                    .place()
                    .is_some_and(|place| place.device == id || place.hermes == id)
            })
            .collect()
    }

    /// Closes every channel connection of a device with `code`.
    pub(crate) fn close_device(&self, device: Uuid, code: ErrorCode) {
        for entry in self.of(device) {
            entry.handle.close(code);
        }
    }
}

/// A connection in the record. Dropped, it leaves the record.
#[derive(Debug)]
pub(crate) struct Registration {
    registry: Arc<Registry>,
    entry: Arc<Entry>,
    turn: Arc<tokio::sync::Mutex<()>>,
}

impl Registration {
    pub(crate) fn entry(&self) -> &Arc<Entry> {
        &self.entry
    }

    /// Waits for the turn of the identity to be judged, and holds it until the guard drops:
    /// from step 5 to the end of T1, one connection of the identity at a time.
    pub(crate) async fn turn(&self) -> OwnedMutexGuard<()> {
        Arc::clone(&self.turn).lock_owned().await
    }

    /// Whether no other connection of the same identity is newer (step 5).
    pub(crate) fn is_newest(&self) -> bool {
        self.registry
            .of_identity(&self.entry.identity)
            .iter()
            .all(|other| other.serial <= self.entry.serial)
    }

    /// The other connections of the same identity.
    pub(crate) fn others(&self) -> Vec<Arc<Entry>> {
        let mut others = self.registry.of_identity(&self.entry.identity);
        others.retain(|other| other.serial != self.entry.serial);
        others
    }

    /// Step 6: the older connections of the identity that are not established yet close
    /// with AR-206, without consequence.
    pub(crate) fn close_older_attempts(&self) {
        for other in self.others() {
            if other.serial < self.entry.serial && other.status() == Status::Attempt {
                other.handle.close(ErrorCode::Ar206);
            }
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.registry.remove(&self.entry);
    }
}
