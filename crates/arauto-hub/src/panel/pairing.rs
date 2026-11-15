//! The pairings the panel runs: one task of the hub process per pairing, holding the QR or
//! the code in memory only (`docs/decisoes/0044-numeros-no-painel-sobre-o-motor.md`).
//!
//! The loop that polls the engine is `admin number pair` itself
//! ([`crate::admin::Session::number_pair_stoppable`]); this module only owns the task around
//! it and what it shows:
//!
//! - **The QR or the code is in memory and nowhere else.** [`Now`] holds it while the pairing
//!   runs and drops it (it is wiped when dropped) the moment the pairing ends. A page asked
//!   afterwards has nothing to draw.
//! - **One run per number, three per organization.** The registry is keyed by organization
//!   and number, and every lookup also checks the organization of the run.
//! - **A read changes nothing here.** The pages read [`Run::with_now`]; only a form starts,
//!   cancels or ends a run, and only the task removes an ended one.
//! - **Closing the window does not cancel.** The run goes on until its deadline.
//!
//! Not guaranteed: a hub that stops or restarts loses its runs (the row of the number waits
//! for the next check of the hub after its deadline), and the QR and the code also live, for
//! a moment, in the buffers of the HTTP stack, a proxy and the browser, which the hub cannot
//! wipe.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use arauto_protocol::names::NumberAlias;
use tokio::sync::watch;
use tokio::task::AbortHandle;
use uuid::Uuid;

use super::numbers::refusal_of;
use crate::admin::whatsapp::{PairMethod, PairOutcome, Shown};
use crate::admin::{self};
use crate::channel::Hub;
use crate::core::number::NumberState;
use crate::db::ResolvedOrg;
use crate::engine::{PairingCode, PairingQr};

/// Most pairings that run at once, per organization.
pub(crate) const PAIRINGS_PER_ORGANIZATION: usize = 3;

/// How long a form that starts a pairing waits for the engine to show its first QR or code.
const FIRST_SHOWN_WAIT: Duration = Duration::from_secs(8);

/// How long a cancel waits for the loop to end by itself before the task is aborted.
const CANCEL_WAIT: Duration = Duration::from_secs(10);

/// How long an ended run stays, so that a window left open reads how it ended.
const ENDED_KEPT: Duration = Duration::from_secs(600);

/// Why a pairing ended with no outcome: one fixed sentence of the screens, and the state the
/// message names, when it names one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Refusal {
    pub sentence: &'static str,
    pub state: Option<NumberState>,
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum End {
    Outcome(PairOutcome),
    Refused(Refusal),
}

/// What a run shows now.
pub(crate) enum Now {
    /// The task started and the engine showed nothing yet.
    Starting,
    /// A QR to scan. `serial` grows with every new QR.
    Qr { qr: PairingQr, serial: u32 },
    /// The code to type on the phone.
    Code { code: PairingCode, serial: u32 },
    /// It ended. No secret is kept.
    Ended(End, Instant),
}

impl std::fmt::Debug for Now {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the QR, never the code.
        f.write_str(match self {
            Now::Starting => "Now::Starting",
            Now::Qr { .. } => "Now::Qr(..)",
            Now::Code { .. } => "Now::Code(..)",
            Now::Ended(..) => "Now::Ended(..)",
        })
    }
}

/// One pairing in progress, or just ended.
pub(crate) struct Run {
    /// Drawn from the system generator: it names the run in an address. It is no secret and
    /// no credential: the panel session decides who may see the QR.
    pub id: Uuid,
    pub organization: ResolvedOrg,
    pub number: Uuid,
    pub alias: NumberAlias,
    /// The panel session that started it: the only one that is shown the QR or the code.
    pub session: Uuid,
    pub method: PairMethod,
    now: Mutex<Now>,
    changed: watch::Sender<u64>,
    stop: watch::Sender<bool>,
    handle: Mutex<Option<AbortHandle>>,
}

impl std::fmt::Debug for Run {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Run")
            .field("id", &self.id)
            .field("alias", &self.alias)
            .finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

impl Run {
    fn new(
        organization: ResolvedOrg,
        number: Uuid,
        alias: NumberAlias,
        session: Uuid,
        method: PairMethod,
    ) -> Arc<Self> {
        Arc::new(Self {
            id: Uuid::new_v4(),
            organization,
            number,
            alias,
            session,
            method,
            now: Mutex::new(Now::Starting),
            changed: watch::channel(0).0,
            stop: watch::channel(false).0,
            handle: Mutex::new(None),
        })
    }

    fn bump(&self) {
        self.changed.send_modify(|n| *n = n.wrapping_add(1));
    }

    /// Whether it still runs.
    pub(crate) fn live(&self) -> bool {
        !matches!(*lock(&self.now), Now::Ended(..))
    }

    /// Reads what the run shows, without keeping it: the closure runs with the lock held and
    /// must not wait.
    pub(crate) fn with_now<R>(&self, read: impl FnOnce(&Now) -> R) -> R {
        read(&lock(&self.now))
    }

    /// What the loop of the pairing shows: a new QR, or the code. Copies are made on
    /// purpose, by name, and wiped when dropped.
    pub(crate) fn show(&self, shown: Shown<'_>) {
        {
            let mut now = lock(&self.now);
            let next = match (&*now, shown) {
                (Now::Ended(..), _) => return,
                (Now::Qr { serial, .. } | Now::Code { serial, .. }, Shown::Qr(qr)) => Now::Qr {
                    qr: qr.duplicate(),
                    serial: serial.saturating_add(1),
                },
                (Now::Qr { serial, .. } | Now::Code { serial, .. }, Shown::Code(code)) => {
                    Now::Code {
                        code: code.duplicate(),
                        serial: serial.saturating_add(1),
                    }
                }
                (_, Shown::Qr(qr)) => Now::Qr {
                    qr: qr.duplicate(),
                    serial: 1,
                },
                (_, Shown::Code(code)) => Now::Code {
                    code: code.duplicate(),
                    serial: 1,
                },
            };
            // The old value is dropped here, and wiped.
            *now = next;
        }
        self.bump();
    }

    /// The pairing ended. Whatever it showed is dropped, and wiped.
    pub(crate) fn end(&self, how: End) {
        {
            let mut now = lock(&self.now);
            if matches!(*now, Now::Ended(..)) {
                return;
            }
            *now = Now::Ended(how, Instant::now());
        }
        self.bump();
    }

    fn finish(&self, result: Result<PairOutcome, admin::AdminError>) {
        self.end(match result {
            Ok(outcome) => End::Outcome(outcome),
            Err(error) => End::Refused(refusal_of(&error)),
        });
    }

    /// Waits until the run shows something or ends, at most `limit`.
    async fn wait_shown(&self, limit: Duration) {
        let mut seen = self.changed.subscribe();
        let _ = tokio::time::timeout(limit, async {
            loop {
                if !matches!(*lock(&self.now), Now::Starting) {
                    return;
                }
                if seen.changed().await.is_err() {
                    return;
                }
            }
        })
        .await;
    }

    /// Asks the loop to stop, waits for it to end, and ends it by force when it does not.
    pub(crate) async fn cancel(&self) {
        self.stop.send_replace(true);
        let mut seen = self.changed.subscribe();
        let ended = tokio::time::timeout(CANCEL_WAIT, async {
            loop {
                if !self.live() {
                    return;
                }
                if seen.changed().await.is_err() {
                    return;
                }
            }
        })
        .await
        .is_ok();
        if !ended {
            if let Some(handle) = lock(&self.handle).take() {
                handle.abort();
            }
            // The row may stay in pairing until the next check of the hub expires it.
            self.end(End::Outcome(PairOutcome::Cancelled));
        }
    }
}

async fn flagged(mut flag: watch::Receiver<bool>) {
    let _ = flag.wait_for(|set| *set).await.is_ok();
}

/// Why a pairing was not started.
#[derive(Debug)]
pub(crate) enum NotStarted {
    /// A pairing of this number is running.
    Running(Arc<Run>),
    /// The organization is at its ceiling.
    Full,
}

/// The runs of the panel, by organization and number.
#[derive(Debug, Default)]
pub(crate) struct Pairings {
    runs: Mutex<HashMap<(Uuid, Uuid), Arc<Run>>>,
}

impl Pairings {
    /// The run kept for a number, live or just ended, of that organization.
    pub(crate) fn of(&self, organization: ResolvedOrg, number: Uuid) -> Option<Arc<Run>> {
        let found = lock(&self.runs)
            .get(&(organization.id(), number))
            .cloned()?;
        (found.organization == organization).then_some(found)
    }

    /// The run with this identifier, of that organization and that number.
    pub(crate) fn by_id(
        &self,
        organization: ResolvedOrg,
        number: Uuid,
        id: Uuid,
    ) -> Option<Arc<Run>> {
        self.of(organization, number).filter(|run| run.id == id)
    }

    /// How many runs of the organization still run.
    #[cfg(test)]
    pub(crate) fn live_count(&self, organization: ResolvedOrg) -> usize {
        lock(&self.runs)
            .values()
            .filter(|run| run.organization == organization && run.live())
            .count()
    }

    fn forget(&self, run: &Run) {
        let mut runs = lock(&self.runs);
        let key = (run.organization.id(), run.number);
        if runs.get(&key).is_some_and(|found| found.id == run.id) {
            runs.remove(&key);
        }
    }

    /// Takes a place for a new run, under one lock: one live run per number and a ceiling per
    /// organization. An ended run of the number, and any that sat ended for long, go.
    fn take(
        &self,
        organization: ResolvedOrg,
        number: Uuid,
        alias: &NumberAlias,
        session: Uuid,
        method: PairMethod,
    ) -> Result<Arc<Run>, NotStarted> {
        let mut runs = lock(&self.runs);
        runs.retain(|_, run| match &*lock(&run.now) {
            Now::Ended(_, at) => at.elapsed() < ENDED_KEPT,
            _ => true,
        });
        if let Some(live) = runs
            .get(&(organization.id(), number))
            .filter(|run| run.live())
        {
            return Err(NotStarted::Running(Arc::clone(live)));
        }
        let live = runs
            .values()
            .filter(|run| run.organization == organization && run.live())
            .count();
        if live >= PAIRINGS_PER_ORGANIZATION {
            return Err(NotStarted::Full);
        }
        let run = Run::new(organization, number, alias.clone(), session, method);
        runs.insert((organization.id(), number), Arc::clone(&run));
        Ok(run)
    }
}

/// Starts a pairing in a task of the hub, and waits a moment for it to show its first QR or
/// code, so the page that follows usually has it already.
pub(crate) async fn start(
    hub: &Arc<Hub>,
    organization: ResolvedOrg,
    admin: Uuid,
    session: Uuid,
    number: Uuid,
    alias: &NumberAlias,
    method: PairMethod,
) -> Result<Arc<Run>, NotStarted> {
    // The panel is what called this: with none, there is nowhere to keep a run.
    let Some(panel) = hub.panel() else {
        return Err(NotStarted::Full);
    };
    let run = panel
        .pairings
        .take(organization, number, alias, session, method)?;
    let task = {
        let hub = Arc::clone(hub);
        let run = Arc::clone(&run);
        tokio::spawn(async move {
            let session = admin::Session::of_panel(
                hub.pool().clone(),
                hub.mark().clone(),
                run.organization,
                admin,
            );
            let settings = hub.engines().settings().clone();
            let clock = hub.clock();
            let wait_stop = flagged(run.stop.subscribe());
            let wait_hub = flagged(hub.stopping());
            let result = session
                .number_pair_stoppable(
                    hub.vault(),
                    run.alias.as_str(),
                    run.method,
                    &settings,
                    &*clock,
                    |shown| run.show(shown),
                    async {
                        tokio::select! {
                            () = wait_stop => {}
                            () = wait_hub => {}
                        }
                    },
                )
                .await;
            if let Err(error) = &result {
                tracing::error!(%error, "a pairing started from the panel failed");
            }
            run.finish(result);
            tokio::time::sleep(ENDED_KEPT).await;
            if let Some(panel) = hub.panel() {
                panel.pairings.forget(&run);
            }
        })
    };
    *lock(&run.handle) = Some(task.abort_handle());
    run.wait_shown(FIRST_SHOWN_WAIT).await;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn organization() -> ResolvedOrg {
        ResolvedOrg::for_tests(Uuid::new_v4())
    }

    fn alias() -> NumberAlias {
        NumberAlias::parse("main").unwrap()
    }

    #[test]
    fn an_ended_run_keeps_no_qr_or_code() {
        let run = Run::new(
            organization(),
            Uuid::new_v4(),
            alias(),
            Uuid::new_v4(),
            PairMethod::Qr,
        );
        let qr = PairingQr::new("2@a-secret-qr-value".to_owned()).unwrap();
        run.show(Shown::Qr(&qr));
        assert!(run.with_now(|now| matches!(now, Now::Qr { serial: 1, .. })));
        run.show(Shown::Qr(&qr));
        assert!(run.with_now(|now| matches!(now, Now::Qr { serial: 2, .. })));
        let code = PairingCode::new("ABCD-EFGH".to_owned()).unwrap();
        run.show(Shown::Code(&code));
        assert!(run.with_now(|now| matches!(now, Now::Code { serial: 3, .. })));
        run.end(End::Outcome(PairOutcome::Expired));
        assert!(run.with_now(|now| matches!(now, Now::Ended(..))));
        assert!(!run.live());
        // Nothing shown after the end brings a secret back.
        run.show(Shown::Qr(&qr));
        assert!(run.with_now(|now| matches!(now, Now::Ended(..))));
        // Whatever the run prints is redacted.
        let printed = format!("{run:?} {:?}", run.with_now(|now| format!("{now:?}")));
        assert!(!printed.contains("secret") && !printed.contains("ABCD"));
    }

    #[test]
    fn the_registry_finds_a_run_only_in_its_organization() {
        let pairings = Pairings::default();
        let (one, other) = (organization(), organization());
        let number = Uuid::new_v4();
        let run = pairings
            .take(one, number, &alias(), Uuid::new_v4(), PairMethod::Qr)
            .unwrap();
        assert!(pairings.of(one, number).is_some());
        assert!(pairings.of(other, number).is_none());
        assert!(pairings.by_id(one, number, run.id).is_some());
        assert!(pairings.by_id(one, number, Uuid::new_v4()).is_none());
        assert!(pairings.by_id(other, number, run.id).is_none());
        assert_eq!(pairings.live_count(one), 1);
        assert_eq!(pairings.live_count(other), 0);
    }

    #[test]
    fn one_run_per_number_and_three_per_organization() {
        let pairings = Pairings::default();
        let org = organization();
        let numbers: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let take = |number| pairings.take(org, number, &alias(), Uuid::new_v4(), PairMethod::Code);
        assert!(take(numbers[0]).is_ok());
        assert!(matches!(take(numbers[0]), Err(NotStarted::Running(_))));
        assert!(take(numbers[1]).is_ok());
        assert!(take(numbers[2]).is_ok());
        assert!(matches!(take(numbers[3]), Err(NotStarted::Full)));
        // Another organization has its own ceiling.
        let other = organization();
        assert!(
            pairings
                .take(other, numbers[3], &alias(), Uuid::new_v4(), PairMethod::Qr)
                .is_ok()
        );
        // An ended run frees its number and its place.
        pairings
            .of(org, numbers[0])
            .unwrap()
            .end(End::Outcome(PairOutcome::Cancelled));
        assert!(take(numbers[3]).is_ok());
        assert!(take(numbers[0]).is_err(), "the ceiling is full again");
    }
}
