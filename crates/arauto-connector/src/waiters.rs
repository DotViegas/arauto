//! The waits of `await-reply` that the leader holds (decision 0048; `docs/11-conector.md`,
//! section 6).
//!
//! A Hermes that asked something and expects an answer runs `arauto-connector await-reply
//! THREAD` as a background process. The process asks the leader, over the local socket, to
//! wait; the leader keeps one [`Waiter`] per wait, lists the threads of its Hermes with the
//! tool `hermes_list_threads` (a listing marks nothing as read) and tells the process when an
//! answer arrived. It never reads a thread: only the numbers of unread notes, the thread
//! identifiers and the Hermes names of the listing pass through here, each checked.
//!
//! **Arrival.** A wait is on thread `T` with a peer. An answer arrived when `T` has unread
//! notes, or when another thread with the same peer has more unread notes than it had when the
//! wait began (a count that falls lowers that baseline).
//!
//! **Two ways to learn of it.**
//! - The poll: one listing every `await_poll_ms` serves every wait. A wait resolved this way
//!   never calls the local API.
//! - The notice: when a peer notice comes from a peer some wait awaits, the listing is made at
//!   once. If a wait resolves, the notice is answered `accepted` without the local API, and its
//!   identifier is kept as handled. The Hermes was told by the waiting process. A grace check
//!   `await_grace_ms` later wakes it through the local API, with the normal sentence, only if
//!   the answer is still unread then.
//! - A notice that comes just after a poll resolved the wait finds no wait: it wakes through
//!   the local API as usual. The Hermes may then be told twice, once by the waiting process
//!   and once by a separate run (`docs/11-conector.md`, section 6: not guaranteed).
//!
//! At most [`WAITS_MAX`] waits are held; the next gets `busy`. A wait that loses its client
//! frees its place. Nothing here logs a thread, a name or a text.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::PEER_LIST_THREADS_MAX;
use arauto_protocol::message::{CallOutcome, Payload};
use arauto_protocol::names::HermesName;
use arauto_protocol::notice::{Notice, NoticeId};
use arauto_protocol::tools::{HERMES_LIST_THREADS, ToolName};
use serde_json::{Value, json};
use tokio::sync::{Notify, mpsc, oneshot};
use uuid::Uuid;

use crate::leader::Shared;
use crate::local::{self, Refusal};
use crate::reply::parse_thread;
use crate::session::Calls;
use crate::settings::{Settings, duration};

/// The most waits one Hermes holds at the same time.
pub(crate) const WAITS_MAX: usize = 8;

/// The threads asked in each listing: the most the hub gives.
const LIST_LIMIT: u32 = PEER_LIST_THREADS_MAX;

/// One thread of the listing: only what the waits use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) thread: Uuid,
    pub(crate) peer: HermesName,
    pub(crate) unread: u64,
}

/// The rows of the content of `hermes_list_threads`. A row out of form is left out.
pub(crate) fn rows_of(content: &Payload) -> Vec<Row> {
    let Some(Value::Array(threads)) = content.as_map().get("threads") else {
        return Vec::new();
    };
    threads
        .iter()
        .filter_map(|row| {
            let thread = parse_thread(row.get("thread")?.as_str()?)?;
            let peer = HermesName::parse(row.get("peer")?.as_str()?).ok()?;
            let unread = row.get("unread")?.as_u64()?;
            Some(Row {
                thread,
                peer,
                unread,
            })
        })
        .take(LIST_LIMIT as usize)
        .collect()
}

/// Lists the threads of this Hermes.
pub(crate) trait Lister: Sync {
    /// The rows, or the code the tool was refused with.
    fn list(&self) -> impl Future<Output = Result<Vec<Row>, ErrorCode>> + Send;
}

fn list_arguments() -> Payload {
    let mut args = serde_json::Map::new();
    args.insert("limit".to_owned(), json!(LIST_LIMIT));
    Payload::new(args)
}

fn rows_from(outcome: CallOutcome) -> Result<Vec<Row>, ErrorCode> {
    match outcome {
        CallOutcome::Ok(content) => Ok(rows_of(&content)),
        CallOutcome::Refused { code, .. } => Err(code),
    }
}

fn list_tool() -> Result<ToolName, ErrorCode> {
    ToolName::parse(HERMES_LIST_THREADS).map_err(|_| ErrorCode::Ar207)
}

impl Lister for Calls {
    async fn list(&self) -> Result<Vec<Row>, ErrorCode> {
        rows_from(self.call(list_tool()?, list_arguments()).await)
    }
}

impl Lister for Shared {
    async fn list(&self) -> Result<Vec<Row>, ErrorCode> {
        rows_from(self.call(list_tool()?, list_arguments()).await)
    }
}

/// The unread notes of each thread of a peer when a wait began. A thread missing counts as 0.
pub(crate) type Thresholds = HashMap<Uuid, u64>;

fn baseline(thresholds: &Thresholds, thread: Uuid) -> u64 {
    thresholds.get(&thread).copied().unwrap_or(0)
}

/// The thread where an answer arrived, if one did.
fn arrival(rows: &[Row], peer: &HermesName, own: Uuid, thresholds: &Thresholds) -> Option<Uuid> {
    if rows.iter().any(|row| row.thread == own && row.unread > 0) {
        return Some(own);
    }
    rows.iter()
        .find(|row| row.peer == *peer && row.unread > baseline(thresholds, row.thread))
        .map(|row| row.thread)
}

/// A count of unread notes that fell lowers its baseline.
fn lower(rows: &[Row], peer: &HermesName, own: Uuid, thresholds: &mut Thresholds) {
    for row in rows {
        if row.peer == *peer
            && row.thread != own
            && let Some(kept) = thresholds.get_mut(&row.thread)
            && row.unread < *kept
        {
            *kept = row.unread;
        }
    }
}

/// An answer that arrived.
#[derive(Debug)]
pub(crate) struct Arrival {
    pub(crate) thread: Uuid,
    pub(crate) peer: HermesName,
    /// Whether a notice of the hub resolved the wait (otherwise a poll did).
    #[cfg_attr(not(feature = "testing"), allow(dead_code))]
    pub(crate) by_notice: bool,
}

/// The times the registry works with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Times {
    pub(crate) poll: Duration,
    pub(crate) grace: Duration,
}

impl Times {
    pub(crate) fn of(settings: &Settings) -> Self {
        Self {
            poll: duration(settings.await_poll_ms),
            grace: duration(settings.await_grace_ms),
        }
    }
}

struct Waiter {
    key: u64,
    own: Uuid,
    peer: HermesName,
    thresholds: Thresholds,
    tell: oneshot::Sender<Arrival>,
}

/// The check, after a notice was taken as delivered, that the answer was read.
#[derive(Debug, Clone)]
pub(crate) struct Grace {
    pub(crate) id: NoticeId,
    pub(crate) notice: Notice,
    peer: HermesName,
    thresholds: Thresholds,
    due: Instant,
    give_up: Instant,
}

impl Grace {
    /// Whether an answer of the peer is still unread.
    pub(crate) fn still_unread(&self, rows: &[Row]) -> bool {
        rows.iter()
            .any(|row| row.peer == self.peer && row.unread > baseline(&self.thresholds, row.thread))
    }
}

#[derive(Default)]
struct Registry {
    reserved: usize,
    waiters: Vec<Waiter>,
    graces: Vec<Grace>,
    next_key: u64,
}

impl Registry {
    /// Resolves every wait that has an arrival in `rows`; the others have their baselines
    /// lowered. The thresholds of the waits that `by_notice` resolved are returned.
    fn settle(&mut self, rows: &[Row], by_notice: Option<&HermesName>) -> Vec<Thresholds> {
        let mut noticed = Vec::new();
        for waiter in std::mem::take(&mut self.waiters) {
            let Waiter {
                key,
                own,
                peer,
                mut thresholds,
                tell,
            } = waiter;
            let Some(thread) = arrival(rows, &peer, own, &thresholds) else {
                lower(rows, &peer, own, &mut thresholds);
                self.waiters.push(Waiter {
                    key,
                    own,
                    peer,
                    thresholds,
                    tell,
                });
                continue;
            };
            let is_notice = by_notice == Some(&peer);
            let told = tell.send(Arrival {
                thread,
                peer: peer.clone(),
                by_notice: is_notice,
            });
            // A wait whose client is gone is only removed.
            if told.is_err() {
                continue;
            }
            if is_notice {
                noticed.push(thresholds);
            }
        }
        noticed
    }
}

/// Every wait the leader holds and the grace checks.
#[derive(Default)]
pub(crate) struct Waiters {
    registry: Mutex<Registry>,
    changed: Notify,
}

/// The place of a wait that is not yet armed. Dropped, it gives the place back.
pub(crate) struct Reservation {
    waiters: Arc<Waiters>,
    live: bool,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if self.live {
            let mut registry = self.waiters.registry();
            registry.reserved = registry.reserved.saturating_sub(1);
        }
    }
}

/// A wait that is armed. Dropped, it is removed.
pub(crate) struct Armed {
    waiters: Arc<Waiters>,
    key: u64,
}

impl Drop for Armed {
    fn drop(&mut self) {
        self.waiters
            .registry()
            .waiters
            .retain(|waiter| waiter.key != self.key);
    }
}

/// The thread of a wait is not among the threads of the listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotFound;

impl Waiters {
    fn registry(&self) -> MutexGuard<'_, Registry> {
        // Plain values: a panic elsewhere leaves them usable.
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Takes a place for one more wait, unless the most are held.
    pub(crate) fn reserve(self: &Arc<Self>) -> Option<Reservation> {
        let mut registry = self.registry();
        if registry.reserved + registry.waiters.len() >= WAITS_MAX {
            return None;
        }
        registry.reserved += 1;
        Some(Reservation {
            waiters: Arc::clone(self),
            live: true,
        })
    }

    /// Arms the wait on `own`, with the baselines of `rows`. An answer that is already unread
    /// resolves it at once.
    pub(crate) fn arm(
        self: &Arc<Self>,
        mut reservation: Reservation,
        own: Uuid,
        rows: &[Row],
    ) -> Result<(Armed, oneshot::Receiver<Arrival>), NotFound> {
        let peer = rows
            .iter()
            .find(|row| row.thread == own)
            .map(|row| row.peer.clone())
            .ok_or(NotFound)?;
        let mut thresholds: Thresholds = rows
            .iter()
            .filter(|row| row.peer == peer)
            .map(|row| (row.thread, row.unread))
            .collect();
        thresholds.insert(own, 0);
        let (tell, told) = oneshot::channel();
        let key = {
            let mut registry = self.registry();
            reservation.live = false;
            registry.reserved = registry.reserved.saturating_sub(1);
            registry.next_key = registry.next_key.wrapping_add(1);
            let key = registry.next_key;
            registry.waiters.push(Waiter {
                key,
                own,
                peer,
                thresholds,
                tell,
            });
            registry.settle(rows, None);
            key
        };
        self.changed.notify_one();
        Ok((
            Armed {
                waiters: Arc::clone(self),
                key,
            },
            told,
        ))
    }

    /// A poll: every wait with an answer is resolved.
    pub(crate) fn apply_poll(&self, rows: &[Row]) {
        self.registry().settle(rows, None);
    }

    /// Whether a wait is held for `peer`.
    pub(crate) fn interested(&self, peer: &HermesName) -> bool {
        self.registry()
            .waiters
            .iter()
            .any(|waiter| waiter.peer == *peer)
    }

    /// A peer notice came, and `rows` is the listing made at once. Returns `true` when the
    /// Hermes was told by a waiting process: the notice is then not to be woken through the
    /// local API, and one grace check is set.
    pub(crate) fn on_notice(
        &self,
        id: NoticeId,
        notice: &Notice,
        rows: &[Row],
        now: Instant,
        times: Times,
    ) -> bool {
        let Notice::Peer { from, .. } = notice else {
            return false;
        };
        let mut registry = self.registry();
        let maps = registry.settle(rows, Some(from));
        if maps.is_empty() {
            return false;
        }
        let mut merged = Thresholds::new();
        let threads: Vec<Uuid> = maps.iter().flat_map(|map| map.keys().copied()).collect();
        for thread in threads {
            let lowest = maps
                .iter()
                .map(|map| baseline(map, thread))
                .min()
                .unwrap_or(0);
            merged.insert(thread, lowest);
        }
        let due = now + times.grace;
        registry.graces.push(Grace {
            id,
            notice: notice.clone(),
            peer: from.clone(),
            thresholds: merged,
            due,
            give_up: due + times.grace,
        });
        drop(registry);
        self.changed.notify_one();
        true
    }

    /// The grace checks that are due at `now`, taken out.
    pub(crate) fn take_due(&self, now: Instant) -> Vec<Grace> {
        let mut registry = self.registry();
        let all = std::mem::take(&mut registry.graces);
        let (due, later): (Vec<Grace>, Vec<Grace>) =
            all.into_iter().partition(|grace| grace.due <= now);
        registry.graces = later;
        due
    }

    /// A grace check that could not be made, kept for a later try until it gives up.
    pub(crate) fn put_back(&self, mut grace: Grace, now: Instant, times: Times) {
        if now < grace.give_up {
            grace.due = now + times.poll;
            self.registry().graces.push(grace);
        }
    }

    /// Whether any wait is held, and when the first grace check is due.
    pub(crate) fn plan(&self) -> (bool, Option<Instant>) {
        let registry = self.registry();
        let due = registry.graces.iter().map(|grace| grace.due).min();
        (!registry.waiters.is_empty(), due)
    }

    #[cfg(test)]
    fn armed(&self) -> usize {
        self.registry().waiters.len()
    }
}

fn tokio_instant(at: Instant) -> tokio::time::Instant {
    tokio::time::Instant::from_std(at)
}

/// The leader's side of the local op `await`, for one connection. It ends when the answer is
/// sent, or when the task is aborted: a closed connection or the end of the leader. In both
/// cases the place is freed by the drop of the reservation or of the armed wait.
pub(crate) async fn serve(
    shared: Arc<Shared>,
    id: u64,
    thread: Uuid,
    minutes: u32,
    out: mpsc::Sender<Vec<u8>>,
) {
    let waiters = &shared.settings.waiters;
    let Some(reservation) = waiters.reserve() else {
        let _ = out
            .send(local::line(&local::refusal(Some(id), Refusal::Busy)))
            .await;
        return;
    };
    let rows = match shared.list().await {
        Ok(rows) => rows,
        Err(code) => {
            let _ = out.send(local::line(&local::await_refused(id, code))).await;
            return;
        }
    };
    let now = Instant::now();
    let deadline = now + duration(shared.settings.await_minute_ms) * minutes;
    let Ok((armed, told)) = waiters.arm(reservation, thread, &rows) else {
        let _ = out
            .send(local::line(&local::await_refused(id, ErrorCode::Ar708)))
            .await;
        return;
    };
    // `biased`: an answer that arrived is announced even when the deadline is also due.
    let line = tokio::select! {
        biased;
        got = told => match got {
            Ok(arrival) => {
                #[cfg(feature = "testing")]
                shared.settings.emit(|| crate::testing::Event::WaitResolved {
                    by_notice: arrival.by_notice,
                });
                local::await_arrived(id, arrival.thread, &arrival.peer)
            }
            Err(_) => return,
        },
        () = tokio::time::sleep_until(tokio_instant(deadline)) => local::await_deadline(id),
    };
    drop(armed);
    let _ = out.send(local::line(&line)).await;
}

/// The loop of the leader: lists the threads every poll interval while a wait is held, at
/// once when a notice asks, and makes the grace checks. It returns when the leader ends.
pub(crate) async fn run(shared: Arc<Shared>) {
    let waiters = &shared.settings.waiters;
    let times = Times::of(&shared.settings);
    let mut finished = shared.finished.subscribe();
    let mut next_poll: Option<Instant> = None;
    loop {
        let now = Instant::now();
        let (any, grace_at) = waiters.plan();
        next_poll = if any {
            next_poll.or(Some(now + times.poll))
        } else {
            None
        };
        let wake_at = [next_poll, grace_at].into_iter().flatten().min();
        tokio::select! {
            () = async {
                match wake_at {
                    Some(at) => tokio::time::sleep_until(tokio_instant(at)).await,
                    None => std::future::pending().await,
                }
            } => {}
            () = waiters.changed.notified() => continue,
            _ = finished.wait_for(Option::is_some) => return,
        }
        let now = Instant::now();
        let poll_due = next_poll.is_some_and(|at| at <= now);
        let graces = waiters.take_due(now);
        if !poll_due && graces.is_empty() {
            continue;
        }
        match shared.list().await {
            Ok(rows) => {
                if poll_due {
                    waiters.apply_poll(&rows);
                    next_poll = Some(now + times.poll);
                }
                for grace in graces {
                    if grace.still_unread(&rows) {
                        shared
                            .settings
                            .waker
                            .wake_late(&shared.settings, grace.id, &grace.notice)
                            .await;
                    }
                }
            }
            Err(_) => {
                if poll_due {
                    next_poll = Some(now + times.poll);
                }
                for grace in graces {
                    waiters.put_back(grace, now, times);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use proptest::prelude::*;

    use super::*;

    fn name(text: &str) -> HermesName {
        HermesName::parse(text).unwrap()
    }

    fn thread(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn row(n: u128, peer: &str, unread: u64) -> Row {
        Row {
            thread: thread(n),
            peer: name(peer),
            unread,
        }
    }

    fn times() -> Times {
        Times {
            poll: Duration::from_secs(30),
            grace: Duration::from_secs(600),
        }
    }

    fn notice(from: &str) -> Notice {
        Notice::Peer {
            from: name(from),
            count: arauto_protocol::notice::NoticeCount::new(1).unwrap(),
            follow: false,
        }
    }

    fn id(n: u128) -> NoticeId {
        NoticeId::new(Uuid::from_u128(n))
    }

    fn payload(value: &Value) -> Payload {
        Payload::new(value.as_object().cloned().unwrap())
    }

    #[test]
    fn rows_out_of_form_are_left_out() {
        let good = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
        let content = payload(&json!({"threads": [
            {"thread": good, "peer": "ana", "unread": 2},
            {"thread": "$(id)", "peer": "ana", "unread": 1},
            {"thread": good, "peer": "Ana", "unread": 1},
            {"thread": good, "peer": "ana", "unread": "1"},
            {"thread": good, "peer": "ana"},
            {"peer": "ana", "unread": 1},
            "text",
        ], "more": false}));
        let rows = rows_of(&content);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows.first().map(|row| row.unread), Some(2));
        assert!(rows_of(&payload(&json!({"threads": "no"}))).is_empty());
        assert!(rows_of(&payload(&json!({}))).is_empty());
    }

    #[test]
    fn the_thread_itself_starts_at_zero_and_the_others_at_their_count() {
        let waiters = Arc::new(Waiters::default());
        let rows = [row(1, "ana", 0), row(2, "ana", 3), row(3, "bento", 5)];
        let reservation = waiters.reserve().unwrap();
        let (_armed, mut told) = waiters.arm(reservation, thread(1), &rows).unwrap();
        assert!(told.try_recv().is_err());
        {
            let registry = waiters.registry();
            let waiter = registry.waiters.first().unwrap();
            assert_eq!(waiter.thresholds.get(&thread(1)), Some(&0));
            assert_eq!(waiter.thresholds.get(&thread(2)), Some(&3));
            assert_eq!(waiter.thresholds.get(&thread(3)), None);
        }
        // A reply that was already unread resolves at once.
        let rows = [row(1, "ana", 1)];
        let reservation = waiters.reserve().unwrap();
        let (_other, mut told) = waiters.arm(reservation, thread(1), &rows).unwrap();
        assert_eq!(told.try_recv().unwrap().thread, thread(1));
        // A thread that is not listed cannot be awaited, and its place is given back.
        let reservation = waiters.reserve().unwrap();
        assert_eq!(
            waiters.arm(reservation, thread(9), &rows).err(),
            Some(NotFound)
        );
        assert_eq!(waiters.registry().reserved, 0);
    }

    #[test]
    fn a_falling_count_lowers_the_baseline() {
        let waiters = Arc::new(Waiters::default());
        let reservation = waiters.reserve().unwrap();
        let (_armed, mut told) = waiters
            .arm(
                reservation,
                thread(1),
                &[row(1, "ana", 0), row(2, "ana", 3)],
            )
            .unwrap();
        // The operator's Hermes read two of the old notes: the baseline is now 1.
        waiters.apply_poll(&[row(1, "ana", 0), row(2, "ana", 1)]);
        assert!(told.try_recv().is_err());
        // One new note in the other thread is above the lowered baseline.
        waiters.apply_poll(&[row(1, "ana", 0), row(2, "ana", 2)]);
        assert_eq!(told.try_recv().unwrap().thread, thread(2));
    }

    #[test]
    fn the_ninth_wait_is_refused_and_a_dropped_one_frees_its_place() {
        let waiters = Arc::new(Waiters::default());
        let rows = [row(1, "ana", 0)];
        let mut held = Vec::new();
        for _ in 0..WAITS_MAX {
            let reservation = waiters.reserve().unwrap();
            held.push(waiters.arm(reservation, thread(1), &rows).unwrap());
        }
        assert!(waiters.reserve().is_none());
        assert_eq!(waiters.armed(), WAITS_MAX);
        drop(held.pop());
        let place = waiters.reserve();
        assert!(place.is_some());
        // A reservation counts too, and an unused one gives its place back.
        assert!(waiters.reserve().is_none());
        drop(place);
        assert!(waiters.reserve().is_some());
    }

    #[test]
    fn a_poll_resolution_never_schedules_a_grace_check() {
        let waiters = Arc::new(Waiters::default());
        let now = Instant::now();
        let reservation = waiters.reserve().unwrap();
        let (_armed, mut told) = waiters
            .arm(reservation, thread(1), &[row(1, "ana", 0)])
            .unwrap();
        waiters.apply_poll(&[row(1, "ana", 1)]);
        let arrival = told.try_recv().unwrap();
        assert!(!arrival.by_notice);
        assert!(waiters.registry().graces.is_empty());
        let (_, due) = waiters.plan();
        assert_eq!(due, None);
        assert!(
            waiters
                .take_due(now + Duration::from_secs(86_400))
                .is_empty()
        );
    }

    #[test]
    fn a_notice_resolution_suppresses_and_schedules_one_grace_check() {
        let waiters = Arc::new(Waiters::default());
        let now = Instant::now();
        let reservation = waiters.reserve().unwrap();
        let (_armed, mut told) = waiters
            .arm(reservation, thread(1), &[row(1, "ana", 0)])
            .unwrap();
        let rows = [row(1, "ana", 1)];
        assert!(waiters.on_notice(id(7), &notice("ana"), &rows, now, times()));
        assert!(told.try_recv().unwrap().by_notice);
        let (any, due) = waiters.plan();
        assert!(!any);
        assert_eq!(due, Some(now + times().grace));
        assert!(waiters.take_due(now).is_empty());
        let due = waiters.take_due(now + times().grace);
        assert_eq!(due.len(), 1);
        let grace = due.first().unwrap();
        assert_eq!(grace.id, id(7));
        // Unread still: it wakes. Read: it does not.
        assert!(grace.still_unread(&rows));
        assert!(!grace.still_unread(&[row(1, "ana", 0)]));
        // A check that could not be made is kept until it gives up.
        let later = now + times().grace;
        waiters.put_back(grace.clone(), later, times());
        assert_eq!(waiters.registry().graces.len(), 1);
        waiters.put_back(grace.clone(), now + times().grace * 3, times());
        assert_eq!(waiters.registry().graces.len(), 1);
    }

    #[test]
    fn a_notice_after_a_poll_resolution_is_not_suppressed() {
        // The poll told the Hermes of the answer in thread 1. A later note of the same peer,
        // in another thread, has a notice of its own: nothing waits for it, so it wakes.
        let now = Instant::now();
        let waiters = Arc::new(Waiters::default());
        let reservation = waiters.reserve().unwrap();
        let (_armed, mut told) = waiters
            .arm(reservation, thread(1), &[row(1, "ana", 0)])
            .unwrap();
        waiters.apply_poll(&[row(1, "ana", 1)]);
        assert!(!told.try_recv().unwrap().by_notice);
        let rows = [row(1, "ana", 1), row(2, "ana", 1)];
        assert!(!waiters.interested(&name("ana")));
        assert!(!waiters.on_notice(id(1), &notice("ana"), &rows, now, times()));
        assert!(waiters.registry().graces.is_empty());
        assert_eq!(waiters.plan(), (false, None));
    }

    #[test]
    fn a_notice_of_another_peer_is_not_suppressed() {
        let waiters = Arc::new(Waiters::default());
        let now = Instant::now();
        let reservation = waiters.reserve().unwrap();
        let (_armed, mut told) = waiters
            .arm(reservation, thread(1), &[row(1, "ana", 0)])
            .unwrap();
        assert!(!waiters.interested(&name("bento")));
        let rows = [row(1, "ana", 0), row(2, "bento", 1)];
        assert!(!waiters.on_notice(id(1), &notice("bento"), &rows, now, times()));
        assert!(told.try_recv().is_err());
        assert_eq!(waiters.armed(), 1);
        // A WhatsApp notice never resolves anything.
        let whatsapp = Notice::Whatsapp {
            number: arauto_protocol::names::NumberAlias::parse("balcao").unwrap(),
            chat: arauto_protocol::names::ChatId::parse("abc").unwrap(),
            count: arauto_protocol::notice::NoticeCount::new(1).unwrap(),
            session: None,
        };
        assert!(!waiters.on_notice(id(2), &whatsapp, &[row(1, "ana", 1)], now, times()));
        assert!(told.try_recv().is_err());
    }

    // ---------------------------------------------------------------------------------
    // Randomized rounds: the registry as a queue, with the decision of `Waker::handle`
    // written out beside it.
    // ---------------------------------------------------------------------------------

    const PEERS: [&str; 3] = ["ana", "bento", "caio"];

    #[derive(Debug, Clone)]
    enum Op {
        Register(u8),
        Reply(u8, u8),
        Deliver(u8),
        Redeliver,
        Poll,
        Read(u8),
        Drop(u8),
        Restart,
        Advance(u16),
    }

    fn ops() -> impl Strategy<Value = Vec<Op>> {
        let op = prop_oneof![
            4 => (0u8..6).prop_map(Op::Register),
            4 => (0u8..4, 0u8..6).prop_map(|(kind, which)| Op::Reply(kind, which)),
            4 => (0u8..3).prop_map(Op::Deliver),
            1 => Just(Op::Redeliver),
            2 => Just(Op::Poll),
            2 => (0u8..6).prop_map(Op::Read),
            2 => (0u8..8).prop_map(Op::Drop),
            1 => Just(Op::Restart),
            3 => (0u16..400).prop_map(Op::Advance),
        ];
        proptest::collection::vec(op, 1..60)
    }

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum Decision {
        Suppressed,
        NormalWake,
    }

    struct World {
        base: Instant,
        elapsed: Duration,
        /// thread n -> (peer index, unread)
        rows: Vec<(u128, usize, u64)>,
        pending: [bool; 3],
        waiters: Arc<Waiters>,
        armed: Vec<(Armed, oneshot::Receiver<Arrival>, Uuid, usize)>,
        handled: HashSet<u128>,
        decisions: HashMap<u128, Decision>,
        api_calls: HashMap<u128, u32>,
        next_id: u128,
        last_id: Option<(u128, usize)>,
        restarted: bool,
    }

    impl World {
        fn now(&self) -> Instant {
            self.base + self.elapsed
        }

        fn listing(&self) -> Vec<Row> {
            self.rows
                .iter()
                .map(|(n, peer, unread)| Row {
                    thread: thread(*n),
                    peer: name(PEERS[*peer]),
                    unread: *unread,
                })
                .collect()
        }

        /// What a resolved wait must satisfy: it was told, and a note was unread.
        fn drain(&mut self) -> Vec<usize> {
            let rows = self.rows.clone();
            let mut resolved = Vec::new();
            let mut kept = Vec::new();
            for (armed, mut told, own, peer) in std::mem::take(&mut self.armed) {
                match told.try_recv() {
                    Ok(arrival) => {
                        resolved.push(peer);
                        let n = arrival.thread.as_u128();
                        let unread = rows.iter().find(|r| r.0 == n).map(|r| r.2).unwrap_or(0);
                        assert!(unread > 0, "no resolution without an unread note");
                        assert_eq!(arrival.peer, name(PEERS[peer]));
                        drop(armed);
                    }
                    Err(oneshot::error::TryRecvError::Empty) => kept.push((armed, told, own, peer)),
                    Err(oneshot::error::TryRecvError::Closed) => drop(armed),
                }
            }
            self.armed = kept;
            resolved
        }

        /// The decision of `Waker::handle` for a peer notice, as written in `wake.rs`.
        fn deliver(&mut self, id: u128, peer: usize) -> Option<Decision> {
            if self.handled.contains(&id) {
                return None;
            }
            let rows = self.listing();
            let now = self.now();
            let own_unread = self.armed.iter().any(|(_, _, own, p)| {
                *p == peer && rows.iter().any(|r| r.thread == *own && r.unread > 0)
            });
            let suppressed = self.waiters.interested(&name(PEERS[peer]))
                && self.waiters.on_notice(
                    NoticeId::new(Uuid::from_u128(id)),
                    &notice(PEERS[peer]),
                    &rows,
                    now,
                    times(),
                );
            self.handled.insert(id);
            let decision = if suppressed {
                Decision::Suppressed
            } else {
                *self.api_calls.entry(id).or_default() += 1;
                Decision::NormalWake
            };
            if own_unread {
                assert_eq!(
                    decision,
                    Decision::Suppressed,
                    "a reply in the awaited thread must resolve its wait"
                );
            }
            let resolved = self.drain();
            if decision == Decision::NormalWake {
                assert!(
                    !resolved.contains(&peer),
                    "a notice that wakes cannot also resolve a wait"
                );
            }
            self.decisions.insert(id, decision);
            Some(decision)
        }

        fn advance(&mut self, ms: u64) {
            self.elapsed += Duration::from_millis(ms);
            let now = self.now();
            let rows = self.listing();
            for grace in self.waiters.take_due(now) {
                let key = grace.id.get().as_u128();
                assert_eq!(
                    self.decisions.get(&key),
                    Some(&Decision::Suppressed),
                    "a grace check only for a suppressed notice"
                );
                if grace.still_unread(&rows) {
                    *self.api_calls.entry(key).or_default() += 1;
                }
            }
        }

        fn check(&self) {
            assert!(self.waiters.armed() <= WAITS_MAX);
            for (id, calls) in &self.api_calls {
                assert!(*calls <= 1, "notice {id} called the API {calls} times");
                assert!(self.handled.contains(id), "no API call without a notice");
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn rounds_of_the_registry_keep_their_invariants(ops in ops()) {
            let mut world = World {
                base: Instant::now(),
                elapsed: Duration::ZERO,
                rows: vec![(1, 0, 0), (2, 0, 0), (3, 1, 0), (4, 2, 0)],
                pending: [false; 3],
                waiters: Arc::new(Waiters::default()),
                armed: Vec::new(),
                handled: HashSet::new(),
                decisions: HashMap::new(),
                api_calls: HashMap::new(),
                next_id: 1_000,
                last_id: None,
                restarted: false,
            };
            for op in ops {
                match op {
                    Op::Register(which) => {
                        let n = u128::from(which) + 1;
                        let listing = world.listing();
                        if let Some(reservation) = world.waiters.reserve() {
                            let peer = world.rows.iter().find(|r| r.0 == n).map(|r| r.1);
                            match world.waiters.arm(reservation, thread(n), &listing) {
                                Ok((armed, told)) => {
                                    world.armed.push((armed, told, thread(n), peer.unwrap_or(0)));
                                    world.drain();
                                }
                                Err(NotFound) => prop_assert!(peer.is_none()),
                            }
                        }
                    }
                    Op::Reply(kind, which) => {
                        let n = u128::from(which) + 1;
                        match kind {
                            // The same thread, or another one of the same peer, or a new
                            // thread, or another peer: all are notes that arrive.
                            0 | 1 => {
                                if let Some(row) = world.rows.iter_mut().find(|r| r.0 == n) {
                                    row.2 += 1;
                                    world.pending[row.1] = true;
                                }
                            }
                            2 => {
                                let peer = usize::from(which) % 3;
                                let new = 10 + u128::from(which);
                                if world.rows.iter().all(|r| r.0 != new) {
                                    world.rows.push((new, peer, 1));
                                } else if let Some(row) = world.rows.iter_mut().find(|r| r.0 == new) {
                                    row.2 += 1;
                                }
                                world.pending[peer] = true;
                            }
                            _ => {
                                if let Some(row) = world.rows.iter_mut().find(|r| r.1 != 0) {
                                    row.2 += 1;
                                    world.pending[row.1] = true;
                                }
                            }
                        }
                    }
                    Op::Deliver(peer) => {
                        let peer = usize::from(peer);
                        if world.pending[peer] {
                            world.pending[peer] = false;
                            world.next_id += 1;
                            let id = world.next_id;
                            world.last_id = Some((id, peer));
                            prop_assert!(world.deliver(id, peer).is_some());
                        }
                    }
                    Op::Redeliver => {
                        if let Some((id, peer)) = world.last_id {
                            // The same identifier again: it was handled, whatever the decision.
                            prop_assert!(world.deliver(id, peer).is_none());
                        }
                    }
                    Op::Poll => {
                        let listing = world.listing();
                        world.waiters.apply_poll(&listing);
                        world.drain();
                    }
                    Op::Read(which) => {
                        let n = u128::from(which) + 1;
                        if let Some(row) = world.rows.iter_mut().find(|r| r.0 == n) {
                            row.2 = 0;
                        }
                    }
                    Op::Drop(which) => {
                        let at = usize::from(which);
                        if at < world.armed.len() {
                            world.armed.remove(at);
                        }
                    }
                    Op::Restart => {
                        // A new process: no waits, no graces; the handled set is on disk.
                        world.armed.clear();
                        world.waiters = Arc::new(Waiters::default());
                        world.restarted = true;
                    }
                    Op::Advance(ms) => world.advance(u64::from(ms) * 10),
                }
                world.check();
            }
        }
    }
}
