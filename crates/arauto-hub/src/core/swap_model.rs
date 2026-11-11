//! A simulator of the credential swap, in memory.
//!
//! One device, one hub, and up to two programs that hold the files of that device: the real
//! connector and, in some rounds, a **copy** of its files used by another process. The hub
//! decides with the pure functions of [`super::judge`], in the order of
//! `docs/14-protocolo-do-canal.md`, section 6: an order number per connection, only the
//! newest one judged, the older ones that are not established closed without consequence,
//! the 3-second probe of an established connection, and T1 and T2 that read again before
//! writing.
//!
//! There is no clock. A deadline is an event that may fire at any moment, and a message on
//! its way may arrive at any moment or never: whatever holds here holds for every timing.
//! Each step is drawn by a [`Chooser`], so a seed replays a round. What is drawn:
//!
//! - the connector or the hub stopping at any point, among them between writing the pending
//!   credential and sending it, and between hearing the "saved" and writing the promotion;
//! - slow connections, and messages of abandoned attempts that arrive late (the late
//!   `check_ok` among them);
//! - a message delivered twice;
//! - the 24-hour swap inside a session;
//! - a copy of the files of the connector, used with the real one on and off, by a process
//!   that behaves, that never confirms, or that presents anything it ever held.
//!
//! After every step the simulator checks four things, and a round ends at the first one
//! that fails ([`Violation`]):
//!
//! - **(a)** a legitimate device that kept its files and is the only one using them never
//!   goes to quarantine and is never locked out;
//! - **(b)** a credential that was already swapped is never accepted;
//! - **(c)** once one holder of the files was served after the copy, the other one is never
//!   accepted: presenting again ends in quarantine, never in silent use by both;
//! - **(d)** the generation only grows, only the pending credential ever becomes the current
//!   one, and the hub never serves two sessions of the device at once.
//!
//! The order rules are in a trait, [`Rules`], whose default methods are the documented
//! rules. A test that overrides one method gets a wrong hub, and shows that the simulator
//! catches it.
//!
//! What is left out: the presentation without swap of a closed device (a closed device gets
//! its code and the round has nothing more to show), and a restored database.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use arauto_protocol::digest::CredentialDigest;
use arauto_protocol::message::Generation;

use super::judge::{
    self, Credential, Decision, DeviceSnapshot, Mode, Presented, QuarantineReason, Transition,
    TransitionError,
};
use super::model::DeviceState;

/// The source of every choice of a round.
pub trait Chooser {
    /// A uniform number in `0..bound`, and 0 when `bound` is 0.
    fn below(&mut self, bound: u64) -> u64;
}

/// The rules the simulated hub follows. The default of each method is the documented rule.
pub trait Rules {
    /// The judgment of a presentation ([`judge::judge`]).
    fn judge(&self, device: &DeviceSnapshot, presented: &Presented, mode: Mode) -> Decision {
        judge::judge(device, presented, mode)
    }

    /// Whether the 24-hour swap may issue ([`judge::rotation`]).
    fn rotation(&self, device: &DeviceSnapshot) -> Decision {
        judge::rotation(device)
    }

    /// The generation of the next credential ([`judge::next_generation`]).
    ///
    /// # Errors
    /// When the device has no generation left.
    fn next_generation(&self, device: &DeviceSnapshot) -> Result<Generation, TransitionError> {
        judge::next_generation(device)
    }

    /// T1 ([`judge::issue`]).
    ///
    /// # Errors
    /// When the transition is refused.
    fn issue(
        &self,
        device: &DeviceSnapshot,
        promote_pending: bool,
        new: Credential,
    ) -> Result<Transition, TransitionError> {
        judge::issue(device, promote_pending, new)
    }

    /// T2 ([`judge::confirm`]).
    ///
    /// # Errors
    /// When the "saved" is not about the pending credential of the connection.
    fn confirm(
        &self,
        device: &DeviceSnapshot,
        issued_here: &Credential,
        saved: &Credential,
    ) -> Result<Transition, TransitionError> {
        judge::confirm(device, issued_here, saved)
    }

    /// Steps 5 and 6: only the newest connection of the device is judged, and judging it
    /// closes the older ones that are not established.
    fn judges_only_the_newest(&self) -> bool {
        true
    }

    /// Step 8: an established connection is probed before another presentation is accepted.
    fn probes_the_established(&self) -> bool {
        true
    }

    /// Rule 4 of C7: the session only starts after the "saved" and the promotion.
    fn serves_only_after_saved(&self) -> bool {
        true
    }
}

/// The rules as documented.
#[derive(Debug, Clone, Copy, Default)]
pub struct Documented;

impl Rules for Documented {}

/// Who holds the files of the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Who {
    /// The connector the device was approved on.
    Real,
    /// Another process, using a copy of the files.
    Copy,
}

impl Who {
    const fn index(self) -> usize {
        match self {
            Who::Real => 0,
            Who::Copy => 1,
        }
    }

    const fn other(self) -> Who {
        match self {
            Who::Real => Who::Copy,
            Who::Copy => Who::Real,
        }
    }
}

/// Which of the four invariants a violation breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Invariant {
    /// A legitimate device alone with its files is never quarantined nor locked out.
    LegitimateDeviceKeepsWorking,
    /// A swapped credential is never accepted.
    SwappedCredentialNeverAccepted,
    /// A copy in use never goes unnoticed when the other holder presents again.
    CopyNeverSilent,
    /// The generation only grows and there is never a second current credential.
    OneLineOfCredentials,
}

/// What the simulator caught.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Violation {
    /// (a) The only holder of the files was put in quarantine.
    LegitimateQuarantined,
    /// (a) The only holder of the files was taken for a second connection.
    LegitimateDoubleConnection,
    /// (a) The only holder of the files was taken for proof of a restored hub.
    LegitimateTakenForRestoration,
    /// (a) With every fault gone, the only holder of the files could not get a session.
    LegitimateLockedOut,
    /// (b) A credential that had already died was accepted.
    SwappedCredentialAccepted,
    /// (b) The identity alone was accepted after a credential had been confirmed.
    NothingAcceptedPastGenerationZero,
    /// (c) A holder was judged after the other one had been served since the copy, and the
    /// outcome was not quarantine.
    SilentUseOfBoth,
    /// (d) A credential was issued with a generation that was not above every earlier one.
    GenerationNotNew,
    /// (d) The largest generation, or the current one, went back.
    GenerationWentBack,
    /// (d) Something that was not the pending credential became the current one.
    CurrentNotFromPending,
    /// (d) Two connections of the device were being served at once.
    TwoSessions,
}

impl Violation {
    /// The invariant this breaks.
    #[must_use]
    pub const fn invariant(self) -> Invariant {
        match self {
            Violation::LegitimateQuarantined
            | Violation::LegitimateDoubleConnection
            | Violation::LegitimateTakenForRestoration
            | Violation::LegitimateLockedOut => Invariant::LegitimateDeviceKeepsWorking,
            Violation::SwappedCredentialAccepted | Violation::NothingAcceptedPastGenerationZero => {
                Invariant::SwappedCredentialNeverAccepted
            }
            Violation::SilentUseOfBoth => Invariant::CopyNeverSilent,
            Violation::GenerationNotNew
            | Violation::GenerationWentBack
            | Violation::CurrentNotFromPending
            | Violation::TwoSessions => Invariant::OneLineOfCredentials,
        }
    }
}

/// A round that broke an invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// What was caught.
    pub violation: Violation,
    /// Every step of the round, in order, for whoever replays the seed.
    pub trace: Vec<String>,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{:?} (invariant {:?}) after {} steps:",
            self.violation,
            self.violation.invariant(),
            self.trace.len()
        )?;
        for (number, step) in self.trace.iter().enumerate() {
            writeln!(f, "  {number:>4} {step}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Failure {}

/// What happened in a round, to show that the rounds reach what they are meant to reach.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    /// Steps taken.
    pub steps: u64,
    /// Rounds in which a copy of the files connected.
    pub rounds_with_copy_in_use: u64,
    /// Sessions that started.
    pub sessions: u64,
    /// 24-hour swaps confirmed.
    pub rotations: u64,
    /// Pending credentials promoted by being presented.
    pub promotions_by_presentation: u64,
    /// Pending credentials discarded by a newer one.
    pub pending_discarded: u64,
    /// Quarantines for reuse.
    pub quarantines: u64,
    /// Quarantines for a double connection.
    pub double_connections: u64,
    /// Connections refused at step 5, because a newer one existed.
    pub not_the_newest: u64,
    /// Connections closed at step 6, when a newer one was judged.
    pub older_closed: u64,
    /// Established connections given up for dead after the probe.
    pub given_up_for_dead: u64,
    /// A "saved" that was not about the pending credential.
    pub saved_refused: u64,
    /// Times the hub stopped.
    pub hub_stops: u64,
    /// Times the hub stopped after writing a pending credential and before sending it.
    pub hub_stops_between_write_and_send: u64,
    /// Times the hub stopped after hearing a "saved" and before writing the promotion.
    pub hub_stops_between_saved_and_promotion: u64,
    /// Times a connector stopped after receiving a credential and before writing it.
    pub connector_stops_before_writing: u64,
    /// Times a connector stopped after writing a credential and before saying "saved".
    pub connector_stops_before_saved: u64,
    /// Messages of an attempt the connector had already abandoned that reached the hub.
    pub late_messages: u64,
    /// Among those, the `check_ok`.
    pub late_check_ok: u64,
    /// Messages delivered twice.
    pub duplicates: u64,
    /// Rounds that ended with the only holder of the files getting a session with every
    /// fault gone.
    pub recoveries: u64,
}

impl Report {
    /// Adds the numbers of another round.
    pub fn add(&mut self, other: &Report) {
        let pairs: [(&mut u64, u64); 21] = [
            (&mut self.steps, other.steps),
            (
                &mut self.rounds_with_copy_in_use,
                other.rounds_with_copy_in_use,
            ),
            (&mut self.sessions, other.sessions),
            (&mut self.rotations, other.rotations),
            (
                &mut self.promotions_by_presentation,
                other.promotions_by_presentation,
            ),
            (&mut self.pending_discarded, other.pending_discarded),
            (&mut self.quarantines, other.quarantines),
            (&mut self.double_connections, other.double_connections),
            (&mut self.not_the_newest, other.not_the_newest),
            (&mut self.older_closed, other.older_closed),
            (&mut self.given_up_for_dead, other.given_up_for_dead),
            (&mut self.saved_refused, other.saved_refused),
            (&mut self.hub_stops, other.hub_stops),
            (
                &mut self.hub_stops_between_write_and_send,
                other.hub_stops_between_write_and_send,
            ),
            (
                &mut self.hub_stops_between_saved_and_promotion,
                other.hub_stops_between_saved_and_promotion,
            ),
            (
                &mut self.connector_stops_before_writing,
                other.connector_stops_before_writing,
            ),
            (
                &mut self.connector_stops_before_saved,
                other.connector_stops_before_saved,
            ),
            (&mut self.late_messages, other.late_messages),
            (&mut self.late_check_ok, other.late_check_ok),
            (&mut self.duplicates, other.duplicates),
            (&mut self.recoveries, other.recoveries),
        ];
        for (total, more) in pairs {
            *total = total.saturating_add(more);
        }
    }
}

/// One thing the simulated hub did to its database, in the order it happened. A test that
/// applies the same steps to the real database must end with the same device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubStep {
    /// The device as the round found it.
    Start(DeviceSnapshot),
    /// T1 of a connection: what was presented, what the rule decided, the credential that
    /// was issued when it accepted, and the device after it.
    Presented {
        /// What the connection presented.
        presented: Option<Credential>,
        /// The decision of the rule.
        decision: Decision,
        /// The new pending credential, when one was written.
        issued: Option<Credential>,
        /// The device after the transaction.
        after: DeviceSnapshot,
    },
    /// T1 of the 24-hour swap.
    Rotated {
        /// The new pending credential.
        issued: Credential,
        /// The device after the transaction.
        after: DeviceSnapshot,
    },
    /// T2: the "saved" of a connection.
    Saved {
        /// The credential that connection had received.
        issued_here: Credential,
        /// What the connector said it wrote.
        saved: Credential,
        /// Whether the promotion was written.
        accepted: bool,
        /// The device after the transaction.
        after: DeviceSnapshot,
    },
    /// A second connection answered its probe: the device went to quarantine.
    DoubleConnection {
        /// The device after the write.
        after: DeviceSnapshot,
    },
}

/// Runs one round: a scenario drawn from `chooser`, then its steps, then, when the device is
/// alone with its files, a quiet end in which it must get a session.
///
/// # Errors
/// [`Failure`] at the first invariant that breaks, with the steps that led to it.
pub fn run_round<R: Rules + ?Sized>(
    rules: &R,
    chooser: &mut dyn Chooser,
) -> Result<Report, Failure> {
    run_round_logged(rules, chooser).1
}

/// [`run_round`], returning also every write the simulated hub made to its database.
pub fn run_round_logged<R: Rules + ?Sized>(
    rules: &R,
    chooser: &mut dyn Chooser,
) -> (Vec<HubStep>, Result<Report, Failure>) {
    let mut world = World::new(rules, chooser);
    let outcome = world.run(chooser);
    (world.log, outcome)
}

// ---------------------------------------------------------------------------- the world

type ConnId = u32;

/// How a copy of the files behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// The connector program itself.
    Honest,
    /// Takes every credential it is given and never says "saved".
    Withholds,
    /// Keeps everything it ever held and presents any of it.
    Replays,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToHub {
    /// The handshake finished: the hub gives the connection its order number.
    Hello,
    Present(Option<Credential>),
    CheckOk(u64),
    Saved(Credential),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Closing {
    /// AR-206: replaced by a newer attempt before becoming a channel.
    Replaced,
    /// AR-208: given up for dead.
    GivenUpForDead,
    /// AR-204, or a deadline of the hub.
    Protocol,
    /// The code of a device that is not approved or active any more.
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToConnector {
    /// `state`: approved or active, the swap follows.
    Swap,
    /// `state`: ended.
    Ended,
    Check(u64),
    Credential(Credential),
    Session,
    Rotate,
    Rotated,
    Closing(Closing),
}

#[derive(Debug)]
struct Wire {
    owner: Who,
    /// Slow connections deliver rarely: their messages are the ones that arrive late.
    slow: bool,
    connector_open: bool,
    hub_open: bool,
    to_hub: VecDeque<ToHub>,
    to_connector: VecDeque<ToConnector>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwapPhase {
    AwaitPresent,
    AwaitCheck {
        presented: Option<Credential>,
        nonce: u64,
    },
    Probing {
        presented: Option<Credential>,
        old: ConnId,
        nonce: u64,
    },
    /// T1 is written. The credential leaves in another step.
    Issued {
        credential: Credential,
        sent: bool,
    },
    /// The "saved" arrived. T2 is another step.
    SavedHeard {
        credential: Credential,
        saved: Credential,
    },
    Idle,
}

#[derive(Debug, Clone, Copy)]
struct HubConn {
    order: u64,
    /// In session: the hub serves tools and notices on it.
    established: bool,
    swap: SwapPhase,
}

#[derive(Debug)]
struct Hub {
    /// The database: what survives a stop of the hub.
    device: DeviceSnapshot,
    /// The registry of connections, in memory.
    conns: BTreeMap<ConnId, HubConn>,
    next_order: u64,
    newest_order: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorPhase {
    AwaitState,
    AwaitCredential,
    /// Received, not on disk yet.
    Writing(Credential),
    /// On disk, "saved" not sent yet.
    Written(Credential),
    AwaitConfirmation,
    Idle,
}

#[derive(Debug, Clone, Copy)]
struct Attempt {
    conn: ConnId,
    phase: ActorPhase,
    in_session: bool,
}

#[derive(Debug)]
struct Actor {
    /// `credential.seal`: what survives a stop of the connector.
    disk: Option<Credential>,
    /// Everything a replaying copy ever held.
    bag: Vec<Option<Credential>>,
    has_files: bool,
    running: bool,
    /// Told the device is closed: it stops using the channel.
    stopped: bool,
    attempt: Option<Attempt>,
    style: Style,
}

#[derive(Debug, Default)]
struct Oracle {
    copy_taken: bool,
    copy_used: bool,
    /// The largest generation issued when the files were copied.
    max_generation_at_copy: u64,
    /// Who was served, since the copy, by a swap of a credential issued after it.
    served_since_copy: [bool; 2],
    /// Credentials that stopped being the current or the pending one.
    dead: Vec<Credential>,
    issued_generations: BTreeSet<u64>,
    ever_current: bool,
}

#[derive(Debug, Clone, Copy)]
enum Event {
    Open(Who),
    Abandon(Who),
    Stop(Who),
    Start(Who),
    WriteToDisk(Who),
    SendSaved(Who),
    ToHub(ConnId),
    ToHubTwice(ConnId),
    ToConnector(ConnId),
    ToConnectorTwice(ConnId),
    HubSendsCredential(ConnId),
    HubPromotes(ConnId),
    HubDeadline(ConnId),
    ProbeDeadline(ConnId),
    HubNoticesClosed(ConnId),
    HubStops,
    Rotate(ConnId),
    CopyFiles,
}

struct World<'a, R: Rules + ?Sized> {
    rules: &'a R,
    hub: Hub,
    actors: [Actor; 2],
    wires: BTreeMap<ConnId, Wire>,
    next_conn: ConnId,
    next_nonce: u64,
    next_digest: u64,
    with_copy: bool,
    oracle: Oracle,
    report: Report,
    violation: Option<Violation>,
    trace: Vec<String>,
    log: Vec<HubStep>,
}

/// How likely each kind of step is, against the others that are possible at that moment.
/// Progress is common and a fault is rare, so that most rounds get somewhere and every fault
/// still lands, over many rounds, at every point.
const WEIGHT_PROGRESS: u64 = 120;
const WEIGHT_SLOW: u64 = 3;
const WEIGHT_OPEN: u64 = 40;
const WEIGHT_START: u64 = 20;
const WEIGHT_FAULT: u64 = 1;
const WEIGHT_TWICE: u64 = 4;
const WEIGHT_NOTICE: u64 = 18;
const WEIGHT_ROTATE: u64 = 6;
const WEIGHT_COPY: u64 = 10;

impl<'a, R: Rules + ?Sized> World<'a, R> {
    fn new(rules: &'a R, chooser: &mut dyn Chooser) -> Self {
        let mut world = Self {
            rules,
            hub: Hub {
                device: DeviceSnapshot {
                    state: DeviceState::Approved,
                    current: None,
                    pending: None,
                    max_generation: 0,
                    marked_by_restoration: false,
                },
                conns: BTreeMap::new(),
                next_order: 1,
                newest_order: 0,
            },
            actors: [
                Actor {
                    disk: None,
                    bag: Vec::new(),
                    has_files: true,
                    running: true,
                    stopped: false,
                    attempt: None,
                    style: Style::Honest,
                },
                Actor {
                    disk: None,
                    bag: Vec::new(),
                    has_files: false,
                    running: false,
                    stopped: false,
                    attempt: None,
                    style: match chooser.below(4) {
                        0 => Style::Withholds,
                        1 => Style::Replays,
                        _ => Style::Honest,
                    },
                },
            ],
            wires: BTreeMap::new(),
            next_conn: 1,
            next_nonce: 1,
            next_digest: 1,
            with_copy: chooser.below(2) == 0,
            oracle: Oracle::default(),
            report: Report::default(),
            violation: None,
            trace: Vec::new(),
            log: Vec::new(),
        };
        // Half of the rounds start with a device already in use, a few swaps in.
        if chooser.below(2) == 0 {
            let generation = 1 + chooser.below(5);
            if let Some(generation) = Generation::new(generation) {
                let current = Credential {
                    generation,
                    digest: world.fresh_digest(),
                };
                world.hub.device = DeviceSnapshot {
                    state: DeviceState::Active,
                    current: Some(current),
                    pending: None,
                    max_generation: generation.get(),
                    marked_by_restoration: false,
                };
                world.actors[Who::Real.index()].disk = Some(current);
                world.oracle.ever_current = true;
                world.oracle.issued_generations.extend(1..=generation.get());
            }
        }
        world.log.push(HubStep::Start(world.hub.device));
        world
    }

    /// The steps of a round, and its quiet end.
    fn run(&mut self, chooser: &mut dyn Chooser) -> Result<Report, Failure> {
        let steps = 60 + chooser.below(240);
        for _ in 0..steps {
            // A device that was closed has nothing more to show: every connection gets its code.
            if !self.hub.device.state.serves() || !self.step(chooser) {
                break;
            }
            if let Some(violation) = self.violation {
                return Err(self.failure(violation));
            }
        }
        if !self.oracle.copy_used && self.hub.device.state.serves() {
            self.settle();
            if let Some(violation) = self.violation {
                return Err(self.failure(violation));
            }
        }
        if self.oracle.copy_used {
            self.report.rounds_with_copy_in_use = 1;
        }
        Ok(self.report)
    }

    fn failure(&self, violation: Violation) -> Failure {
        Failure {
            violation,
            trace: self.trace.clone(),
        }
    }

    fn fresh_digest(&mut self) -> CredentialDigest {
        let mut bytes = [0u8; 32];
        for (target, source) in bytes.iter_mut().zip(self.next_digest.to_be_bytes()) {
            *target = source;
        }
        self.next_digest = self.next_digest.wrapping_add(1);
        CredentialDigest::from_bytes(bytes)
    }

    fn fresh_nonce(&mut self) -> u64 {
        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce.wrapping_add(1);
        nonce
    }

    fn violate(&mut self, violation: Violation) {
        if self.violation.is_none() {
            self.violation = Some(violation);
        }
    }

    fn actor(&mut self, who: Who) -> &mut Actor {
        &mut self.actors[who.index()]
    }

    // ------------------------------------------------------------------ choosing a step

    fn enabled(&self) -> Vec<(Event, u64)> {
        let mut events = Vec::new();
        for who in [Who::Real, Who::Copy] {
            let actor = &self.actors[who.index()];
            if !actor.has_files {
                continue;
            }
            if !actor.running {
                events.push((Event::Start(who), WEIGHT_START));
                continue;
            }
            events.push((Event::Stop(who), WEIGHT_FAULT));
            match actor.attempt {
                None if !actor.stopped => events.push((Event::Open(who), WEIGHT_OPEN)),
                None => {}
                Some(attempt) => {
                    events.push((Event::Abandon(who), WEIGHT_FAULT));
                    match attempt.phase {
                        ActorPhase::Writing(_) => {
                            events.push((Event::WriteToDisk(who), WEIGHT_PROGRESS));
                        }
                        ActorPhase::Written(_) => {
                            events.push((Event::SendSaved(who), WEIGHT_PROGRESS));
                        }
                        _ => {}
                    }
                }
            }
        }
        for (id, wire) in &self.wires {
            let weight = if wire.slow {
                WEIGHT_SLOW
            } else {
                WEIGHT_PROGRESS
            };
            if !wire.to_hub.is_empty() {
                events.push((Event::ToHub(*id), weight));
                if self.hub.conns.contains_key(id) {
                    events.push((Event::ToHubTwice(*id), WEIGHT_TWICE));
                }
            }
            if !wire.to_connector.is_empty() {
                events.push((Event::ToConnector(*id), weight));
                if wire.connector_open {
                    events.push((Event::ToConnectorTwice(*id), WEIGHT_TWICE));
                }
            }
        }
        for (id, conn) in &self.hub.conns {
            match conn.swap {
                SwapPhase::Issued { sent: false, .. } => {
                    events.push((Event::HubSendsCredential(*id), WEIGHT_PROGRESS));
                }
                SwapPhase::SavedHeard { .. } => {
                    events.push((Event::HubPromotes(*id), WEIGHT_PROGRESS));
                }
                SwapPhase::Probing { .. } => {
                    events.push((Event::ProbeDeadline(*id), WEIGHT_NOTICE));
                }
                SwapPhase::AwaitPresent
                | SwapPhase::AwaitCheck { .. }
                | SwapPhase::Issued { sent: true, .. } => {
                    events.push((Event::HubDeadline(*id), WEIGHT_FAULT));
                }
                SwapPhase::Idle => {
                    if conn.established {
                        events.push((Event::Rotate(*id), WEIGHT_ROTATE));
                    }
                }
            }
            if self.wires.get(id).is_some_and(|wire| !wire.connector_open) {
                events.push((Event::HubNoticesClosed(*id), WEIGHT_NOTICE));
            }
        }
        events.push((Event::HubStops, WEIGHT_FAULT));
        if self.with_copy && !self.oracle.copy_taken {
            events.push((Event::CopyFiles, WEIGHT_COPY));
        }
        events
    }

    /// Takes one step. `false` when nothing can happen any more.
    fn step(&mut self, chooser: &mut dyn Chooser) -> bool {
        let events = self.enabled();
        let total: u64 = events.iter().map(|(_, weight)| *weight).sum();
        if total == 0 {
            return false;
        }
        let mut drawn = chooser.below(total);
        let mut chosen = None;
        for (event, weight) in events {
            if drawn < weight {
                chosen = Some(event);
                break;
            }
            drawn -= weight;
        }
        let Some(event) = chosen else {
            return false;
        };
        self.apply(event, chooser);
        true
    }

    fn apply(&mut self, event: Event, chooser: &mut dyn Chooser) {
        self.report.steps = self.report.steps.saturating_add(1);
        self.trace.push(format!("{event:?}"));
        match event {
            Event::Open(who) => self.open(who, chooser),
            Event::Abandon(who) => self.abandon(who),
            Event::Stop(who) => self.stop_actor(who),
            Event::Start(who) => self.actor(who).running = true,
            Event::WriteToDisk(who) => self.write_to_disk(who),
            Event::SendSaved(who) => self.send_saved(who),
            Event::ToHub(conn) => self.deliver_to_hub(conn, true),
            Event::ToHubTwice(conn) => {
                self.report.duplicates = self.report.duplicates.saturating_add(1);
                self.deliver_to_hub(conn, false);
            }
            Event::ToConnector(conn) => self.deliver_to_connector(conn, true, chooser),
            Event::ToConnectorTwice(conn) => {
                self.report.duplicates = self.report.duplicates.saturating_add(1);
                self.deliver_to_connector(conn, false, chooser);
            }
            Event::HubSendsCredential(conn) => self.hub_sends_credential(conn),
            Event::HubPromotes(conn) => self.hub_promotes(conn),
            Event::HubDeadline(conn) => self.hub_closes(conn, Closing::Protocol),
            Event::ProbeDeadline(conn) => self.probe_deadline(conn),
            Event::HubNoticesClosed(conn) => {
                self.hub.conns.remove(&conn);
                if let Some(wire) = self.wires.get_mut(&conn) {
                    wire.hub_open = false;
                }
            }
            Event::HubStops => self.hub_stops(),
            Event::Rotate(conn) => self.rotate(conn),
            Event::CopyFiles => self.copy_files(),
        }
        self.sweep();
        let sessions = self
            .hub
            .conns
            .values()
            .filter(|conn| conn.established)
            .count();
        if sessions > 1 {
            self.violate(Violation::TwoSessions);
        }
    }

    /// Forgets the connections that are closed on both ends with nothing on the way.
    fn sweep(&mut self) {
        self.wires.retain(|_, wire| {
            wire.connector_open
                || wire.hub_open
                || !wire.to_hub.is_empty()
                || !wire.to_connector.is_empty()
        });
        // A message towards an end that is closed never arrives.
        for wire in self.wires.values_mut() {
            if !wire.connector_open {
                wire.to_connector.clear();
            }
        }
    }

    // ------------------------------------------------------------------ the connectors

    fn open(&mut self, who: Who, chooser: &mut dyn Chooser) {
        let conn = self.next_conn;
        self.next_conn = self.next_conn.wrapping_add(1);
        let mut to_hub = VecDeque::new();
        to_hub.push_back(ToHub::Hello);
        self.wires.insert(
            conn,
            Wire {
                owner: who,
                slow: chooser.below(5) == 0,
                connector_open: true,
                hub_open: true,
                to_hub,
                to_connector: VecDeque::new(),
            },
        );
        self.actor(who).attempt = Some(Attempt {
            conn,
            phase: ActorPhase::AwaitState,
            in_session: false,
        });
        if who == Who::Copy {
            self.oracle.copy_used = true;
        }
    }

    /// The connector ends its attempt: its end of the connection closes. What it already
    /// sent may still arrive.
    fn abandon(&mut self, who: Who) {
        if let Some(attempt) = self.actor(who).attempt.take()
            && let Some(wire) = self.wires.get_mut(&attempt.conn)
        {
            wire.connector_open = false;
        }
    }

    fn stop_actor(&mut self, who: Who) {
        match self.actor(who).attempt.map(|attempt| attempt.phase) {
            Some(ActorPhase::Writing(_)) => {
                self.report.connector_stops_before_writing =
                    self.report.connector_stops_before_writing.saturating_add(1);
            }
            Some(ActorPhase::Written(_)) => {
                self.report.connector_stops_before_saved =
                    self.report.connector_stops_before_saved.saturating_add(1);
            }
            _ => {}
        }
        self.abandon(who);
        self.actor(who).running = false;
    }

    fn write_to_disk(&mut self, who: Who) {
        let actor = self.actor(who);
        if let Some(attempt) = actor.attempt.as_mut()
            && let ActorPhase::Writing(credential) = attempt.phase
        {
            attempt.phase = ActorPhase::Written(credential);
            actor.disk = Some(credential);
        }
    }

    fn send_saved(&mut self, who: Who) {
        let Some(attempt) = self.actor(who).attempt.as_mut() else {
            return;
        };
        let ActorPhase::Written(credential) = attempt.phase else {
            return;
        };
        attempt.phase = ActorPhase::AwaitConfirmation;
        let conn = attempt.conn;
        self.send_to_hub(conn, ToHub::Saved(credential));
    }

    fn send_to_hub(&mut self, conn: ConnId, message: ToHub) {
        if let Some(wire) = self.wires.get_mut(&conn) {
            wire.to_hub.push_back(message);
        }
    }

    fn send_to_connector(&mut self, conn: ConnId, message: ToConnector) {
        if let Some(wire) = self.wires.get_mut(&conn)
            && wire.connector_open
        {
            wire.to_connector.push_back(message);
        }
    }

    fn deliver_to_connector(&mut self, conn: ConnId, consume: bool, chooser: &mut dyn Chooser) {
        let Some(wire) = self.wires.get_mut(&conn) else {
            return;
        };
        let message = if consume {
            wire.to_connector.pop_front()
        } else {
            wire.to_connector.front().copied()
        };
        let (Some(message), true) = (message, wire.connector_open) else {
            return;
        };
        let who = wire.owner;
        // Only the attempt in use is listened to: the connector ends one before opening
        // another, and what arrives for an ended one is dropped.
        let Some(attempt) = self.actor(who).attempt.filter(|a| a.conn == conn) else {
            return;
        };
        let mut next = attempt;
        let mut protocol_error = false;
        match message {
            ToConnector::Swap if attempt.phase == ActorPhase::AwaitState => {
                let presented = self.presentation(who, chooser);
                self.send_to_hub(conn, ToHub::Present(presented));
                next.phase = ActorPhase::AwaitCredential;
            }
            ToConnector::Ended | ToConnector::Closing(Closing::Ended) => {
                self.abandon(who);
                self.actor(who).stopped = true;
                return;
            }
            ToConnector::Closing(_) => {
                self.abandon(who);
                return;
            }
            ToConnector::Check(nonce) => self.send_to_hub(conn, ToHub::CheckOk(nonce)),
            ToConnector::Credential(credential) if attempt.phase == ActorPhase::AwaitCredential => {
                let actor = self.actor(who);
                actor.bag.push(Some(credential));
                if actor.style != Style::Withholds {
                    next.phase = ActorPhase::Writing(credential);
                }
            }
            ToConnector::Session if !attempt.in_session => {
                next.in_session = true;
                if attempt.phase == ActorPhase::AwaitConfirmation {
                    next.phase = ActorPhase::Idle;
                }
            }
            ToConnector::Rotate if attempt.in_session && attempt.phase == ActorPhase::Idle => {
                next.phase = ActorPhase::AwaitCredential;
            }
            ToConnector::Rotated
                if attempt.in_session && attempt.phase == ActorPhase::AwaitConfirmation =>
            {
                next.phase = ActorPhase::Idle;
            }
            _ => protocol_error = true,
        }
        if protocol_error {
            self.abandon(who);
        } else {
            self.actor(who).attempt = Some(next);
        }
    }

    /// What a connector presents: what is on its disk. A replaying copy presents anything it
    /// ever held.
    fn presentation(&mut self, who: Who, chooser: &mut dyn Chooser) -> Option<Credential> {
        let actor = &self.actors[who.index()];
        if actor.style == Style::Replays && !actor.bag.is_empty() {
            let bound = u64::try_from(actor.bag.len()).unwrap_or(u64::MAX);
            let chosen = usize::try_from(chooser.below(bound)).unwrap_or(0);
            return actor.bag.get(chosen).copied().flatten();
        }
        actor.disk
    }

    fn copy_files(&mut self) {
        let disk = self.actors[Who::Real.index()].disk;
        let copy = self.actor(Who::Copy);
        copy.disk = disk;
        copy.bag = vec![disk, None];
        copy.has_files = true;
        self.oracle.copy_taken = true;
        self.oracle.max_generation_at_copy = self.hub.device.max_generation;
    }

    // ------------------------------------------------------------------ the hub

    fn deliver_to_hub(&mut self, conn: ConnId, consume: bool) {
        let Some(wire) = self.wires.get_mut(&conn) else {
            return;
        };
        let message = if consume {
            wire.to_hub.pop_front()
        } else {
            wire.to_hub.front().copied()
        };
        let (Some(message), true) = (message, wire.hub_open) else {
            return;
        };
        let late = !wire.connector_open;
        let known = self.hub.conns.contains_key(&conn);
        if late && known {
            self.report.late_messages = self.report.late_messages.saturating_add(1);
            if matches!(message, ToHub::CheckOk(_)) {
                self.report.late_check_ok = self.report.late_check_ok.saturating_add(1);
            }
        }
        match (message, self.hub.conns.get(&conn).map(|c| c.swap)) {
            (ToHub::Hello, None) => self.hello(conn),
            // The hub stopped and came back: it knows nothing of this connection.
            (_, None) => {
                self.send_to_connector(conn, ToConnector::Closing(Closing::Protocol));
                if let Some(wire) = self.wires.get_mut(&conn) {
                    wire.hub_open = false;
                }
            }
            (ToHub::Present(presented), Some(SwapPhase::AwaitPresent)) => {
                let nonce = self.fresh_nonce();
                self.set_swap(conn, SwapPhase::AwaitCheck { presented, nonce });
                self.send_to_connector(conn, ToConnector::Check(nonce));
            }
            (ToHub::CheckOk(answer), Some(SwapPhase::AwaitCheck { presented, nonce }))
                if answer == nonce =>
            {
                self.checked(conn, presented);
            }
            (ToHub::CheckOk(answer), Some(_)) => {
                let probing = self
                    .hub
                    .conns
                    .iter()
                    .find_map(|(id, other)| match other.swap {
                        SwapPhase::Probing { old, nonce, .. } if old == conn && nonce == answer => {
                            Some(*id)
                        }
                        _ => None,
                    });
                match probing {
                    Some(_) => self.double_connection(),
                    None => self.hub_closes(conn, Closing::Protocol),
                }
            }
            (
                ToHub::Saved(saved),
                Some(SwapPhase::Issued {
                    credential,
                    sent: true,
                }),
            ) => self.set_swap(conn, SwapPhase::SavedHeard { credential, saved }),
            // Anything out of its turn is a protocol error: the connection closes, and
            // nothing else follows from it.
            (_, Some(_)) => self.hub_closes(conn, Closing::Protocol),
        }
    }

    fn set_swap(&mut self, conn: ConnId, swap: SwapPhase) {
        if let Some(entry) = self.hub.conns.get_mut(&conn) {
            entry.swap = swap;
        }
    }

    fn hello(&mut self, conn: ConnId) {
        if !self.hub.device.state.serves() {
            self.send_to_connector(conn, ToConnector::Ended);
            if let Some(wire) = self.wires.get_mut(&conn) {
                wire.hub_open = false;
            }
            return;
        }
        let order = self.hub.next_order;
        self.hub.next_order = self.hub.next_order.saturating_add(1);
        self.hub.newest_order = order;
        self.hub.conns.insert(
            conn,
            HubConn {
                order,
                established: false,
                swap: SwapPhase::AwaitPresent,
            },
        );
        self.send_to_connector(conn, ToConnector::Swap);
    }

    /// Closes a connection from the side of the hub, saying why when the other end listens.
    fn hub_closes(&mut self, conn: ConnId, why: Closing) {
        if self.hub.conns.remove(&conn).is_some() {
            self.send_to_connector(conn, ToConnector::Closing(why));
            if let Some(wire) = self.wires.get_mut(&conn) {
                wire.hub_open = false;
            }
        }
    }

    fn close_every_connection(&mut self, why: Closing) {
        let all: Vec<ConnId> = self.hub.conns.keys().copied().collect();
        for conn in all {
            self.hub_closes(conn, why);
        }
    }

    /// Steps 5 to 8, for a connection that answered its check.
    fn checked(&mut self, conn: ConnId, presented: Option<Credential>) {
        let Some(order) = self.hub.conns.get(&conn).map(|entry| entry.order) else {
            return;
        };
        if self.rules.judges_only_the_newest() {
            if order != self.hub.newest_order {
                self.report.not_the_newest = self.report.not_the_newest.saturating_add(1);
                self.hub_closes(conn, Closing::Replaced);
                return;
            }
            let older: Vec<ConnId> = self
                .hub
                .conns
                .iter()
                .filter(|(_, other)| other.order < order && !other.established)
                .map(|(id, _)| *id)
                .collect();
            for other in older {
                self.report.older_closed = self.report.older_closed.saturating_add(1);
                self.hub_closes(other, Closing::Replaced);
            }
        }
        let decision = self
            .rules
            .judge(&self.hub.device, &as_presented(presented), Mode::Swap);
        if matches!(decision, Decision::Accept { .. }) && self.rules.probes_the_established() {
            let established = self
                .hub
                .conns
                .iter()
                .find(|(id, other)| **id != conn && other.established)
                .map(|(id, _)| *id);
            if let Some(old) = established {
                let nonce = self.fresh_nonce();
                self.set_swap(
                    conn,
                    SwapPhase::Probing {
                        presented,
                        old,
                        nonce,
                    },
                );
                self.send_to_connector(old, ToConnector::Check(nonce));
                return;
            }
        }
        self.first_transaction(conn, presented);
    }

    fn probe_deadline(&mut self, conn: ConnId) {
        let Some(SwapPhase::Probing { presented, old, .. }) =
            self.hub.conns.get(&conn).map(|entry| entry.swap)
        else {
            return;
        };
        if self.hub.conns.contains_key(&old) {
            self.report.given_up_for_dead = self.report.given_up_for_dead.saturating_add(1);
            self.hub_closes(old, Closing::GivenUpForDead);
        }
        self.first_transaction(conn, presented);
    }

    fn double_connection(&mut self) {
        self.report.double_connections = self.report.double_connections.saturating_add(1);
        if !self.oracle.copy_used {
            self.violate(Violation::LegitimateDoubleConnection);
        }
        self.end_device(DeviceState::Quarantined);
        self.log.push(HubStep::DoubleConnection {
            after: self.hub.device,
        });
    }

    fn end_device(&mut self, state: DeviceState) {
        let mut after = self.hub.device;
        after.state = state;
        self.write_device(after);
        self.close_every_connection(Closing::Ended);
    }

    /// T1: reads the device again, judges again and writes.
    fn first_transaction(&mut self, conn: ConnId, presented: Option<Credential>) {
        let Some(owner) = self.wires.get(&conn).map(|wire| wire.owner) else {
            return;
        };
        let before = self.hub.device;
        let decision = self
            .rules
            .judge(&before, &as_presented(presented), Mode::Swap);
        self.watch_judgment(owner, presented, &before, decision);
        let mut written = None;
        match decision {
            Decision::Accept { promote_pending } => {
                let issued = self.issue(&before, promote_pending);
                written = issued;
                match issued {
                    Some(credential) => {
                        if promote_pending {
                            self.report.promotions_by_presentation =
                                self.report.promotions_by_presentation.saturating_add(1);
                        }
                        self.set_swap(
                            conn,
                            SwapPhase::Issued {
                                credential,
                                sent: false,
                            },
                        );
                    }
                    None => self.hub_closes(conn, Closing::Protocol),
                }
            }
            Decision::Quarantine(reason) => {
                if reason == QuarantineReason::Reuse {
                    self.report.quarantines = self.report.quarantines.saturating_add(1);
                }
                self.end_device(DeviceState::Quarantined);
            }
            Decision::RestorationProof => {
                self.violate(Violation::LegitimateTakenForRestoration);
                self.end_device(DeviceState::Suspended);
            }
            Decision::ReuseAfterClosing | Decision::RecordOnly | Decision::Nothing => {
                self.hub_closes(conn, Closing::Ended);
            }
        }
        self.log.push(HubStep::Presented {
            presented,
            decision,
            issued: written,
            after: self.hub.device,
        });
    }

    /// Writes a new pending credential, and returns it.
    fn issue(&mut self, before: &DeviceSnapshot, promote_pending: bool) -> Option<Credential> {
        let generation = self.rules.next_generation(before).ok()?;
        let credential = Credential {
            generation,
            digest: self.fresh_digest(),
        };
        let after = self.rules.issue(before, promote_pending, credential).ok()?;
        if !self.oracle.issued_generations.insert(generation.get()) {
            self.violate(Violation::GenerationNotNew);
        }
        self.write_device(after.device);
        Some(credential)
    }

    fn hub_sends_credential(&mut self, conn: ConnId) {
        let Some(entry) = self.hub.conns.get(&conn).copied() else {
            return;
        };
        let SwapPhase::Issued {
            credential,
            sent: false,
        } = entry.swap
        else {
            return;
        };
        self.set_swap(
            conn,
            SwapPhase::Issued {
                credential,
                sent: true,
            },
        );
        self.send_to_connector(conn, ToConnector::Credential(credential));
        if !entry.established && !self.rules.serves_only_after_saved() {
            self.begin_session(conn, &credential);
        }
    }

    /// T2: reads the device again and promotes, if the "saved" is about the pending
    /// credential of this connection.
    fn hub_promotes(&mut self, conn: ConnId) {
        let Some(entry) = self.hub.conns.get(&conn).copied() else {
            return;
        };
        let SwapPhase::SavedHeard { credential, saved } = entry.swap else {
            return;
        };
        let before = self.hub.device;
        let confirmed = self.rules.confirm(&before, &credential, &saved);
        if let Ok(after) = &confirmed {
            self.write_device(after.device);
        }
        self.log.push(HubStep::Saved {
            issued_here: credential,
            saved,
            accepted: confirmed.is_ok(),
            after: self.hub.device,
        });
        match confirmed {
            Ok(_) => {
                self.set_swap(conn, SwapPhase::Idle);
                if entry.established {
                    self.report.rotations = self.report.rotations.saturating_add(1);
                    self.mark_served(conn, &credential);
                    self.send_to_connector(conn, ToConnector::Rotated);
                } else {
                    self.begin_session(conn, &credential);
                }
            }
            Err(_) => {
                self.report.saved_refused = self.report.saved_refused.saturating_add(1);
                self.hub_closes(conn, Closing::Protocol);
            }
        }
    }

    fn begin_session(&mut self, conn: ConnId, credential: &Credential) {
        if let Some(entry) = self.hub.conns.get_mut(&conn) {
            entry.established = true;
        }
        self.report.sessions = self.report.sessions.saturating_add(1);
        self.mark_served(conn, credential);
        self.send_to_connector(conn, ToConnector::Session);
    }

    /// Records who was served by a swap of a credential issued after the copy: from then on
    /// the other holder of the files has nothing that is still alive.
    fn mark_served(&mut self, conn: ConnId, credential: &Credential) {
        if self.oracle.copy_taken
            && credential.generation.get() > self.oracle.max_generation_at_copy
            && let Some(owner) = self.wires.get(&conn).map(|wire| wire.owner)
        {
            self.oracle.served_since_copy[owner.index()] = true;
        }
    }

    fn rotate(&mut self, conn: ConnId) {
        let before = self.hub.device;
        if !matches!(self.rules.rotation(&before), Decision::Accept { .. }) {
            return;
        }
        if let Some(credential) = self.issue(&before, false) {
            self.log.push(HubStep::Rotated {
                issued: credential,
                after: self.hub.device,
            });
            self.send_to_connector(conn, ToConnector::Rotate);
            self.set_swap(
                conn,
                SwapPhase::Issued {
                    credential,
                    sent: false,
                },
            );
        }
    }

    /// The hub process stops and starts again: the registry of connections is gone, the
    /// database stays. What it had already sent may still arrive.
    fn hub_stops(&mut self) {
        self.report.hub_stops = self.report.hub_stops.saturating_add(1);
        for conn in self.hub.conns.values() {
            match conn.swap {
                SwapPhase::Issued { sent: false, .. } => {
                    self.report.hub_stops_between_write_and_send = self
                        .report
                        .hub_stops_between_write_and_send
                        .saturating_add(1);
                }
                SwapPhase::SavedHeard { .. } => {
                    self.report.hub_stops_between_saved_and_promotion = self
                        .report
                        .hub_stops_between_saved_and_promotion
                        .saturating_add(1);
                }
                _ => {}
            }
        }
        self.hub.conns.clear();
        self.hub.next_order = 1;
        self.hub.newest_order = 0;
        for wire in self.wires.values_mut() {
            if wire.hub_open && wire.connector_open {
                // The connector sees its connection drop, after whatever was already sent.
                wire.to_connector
                    .push_back(ToConnector::Closing(Closing::Protocol));
            }
            wire.hub_open = false;
            wire.to_hub.clear();
        }
    }

    // ------------------------------------------------------------------ the invariants

    /// Looks at a judgment that is about to be acted on.
    fn watch_judgment(
        &mut self,
        owner: Who,
        presented: Option<Credential>,
        before: &DeviceSnapshot,
        decision: Decision,
    ) {
        let accepted = matches!(decision, Decision::Accept { .. });
        // (a)
        if !self.oracle.copy_used && matches!(decision, Decision::Quarantine(_)) {
            self.violate(Violation::LegitimateQuarantined);
        }
        // (b)
        if accepted {
            match presented {
                Some(credential) if self.oracle.dead.contains(&credential) => {
                    self.violate(Violation::SwappedCredentialAccepted);
                }
                None if self.oracle.ever_current => {
                    self.violate(Violation::NothingAcceptedPastGenerationZero);
                }
                _ => {}
            }
        }
        // (c)
        if self.oracle.served_since_copy[owner.other().index()]
            && before.state.serves()
            && !matches!(decision, Decision::Quarantine(_))
        {
            self.violate(Violation::SilentUseOfBoth);
        }
    }

    /// Every write of the device goes through here: the line of credentials is checked from
    /// the outside, by what changed, whatever rule made the change.
    fn write_device(&mut self, after: DeviceSnapshot) {
        let before = self.hub.device;
        // (d) the generation only grows.
        let current_generation =
            |device: &DeviceSnapshot| device.current.map_or(0, |c| c.generation.get());
        if after.max_generation < before.max_generation
            || current_generation(&after) < current_generation(&before)
        {
            self.violate(Violation::GenerationWentBack);
        }
        if after.pending != before.pending
            && let Some(pending) = after.pending
            && (pending.generation.get() <= before.max_generation
                || after.max_generation < pending.generation.get())
        {
            self.violate(Violation::GenerationNotNew);
        }
        // (d) only the pending credential becomes the current one.
        if after.current != before.current {
            if after.current.is_none() || after.current != before.pending {
                self.violate(Violation::CurrentNotFromPending);
            }
            if let Some(died) = before.current {
                self.oracle.dead.push(died);
            }
            self.oracle.ever_current = true;
        }
        if after.pending != before.pending
            && let Some(left) = before.pending
            && after.current != Some(left)
        {
            self.oracle.dead.push(left);
            self.report.pending_discarded = self.report.pending_discarded.saturating_add(1);
        }
        if let Some(current) = after.current
            && self.oracle.dead.contains(&current)
        {
            self.violate(Violation::CurrentNotFromPending);
        }
        self.hub.device = after;
    }

    /// The quiet end of a round in which the device is alone with its files: every fault
    /// stops, the connections that were open are gone, and the connector tries once more.
    /// It must get a session.
    fn settle(&mut self) {
        self.trace.push("Settle".to_owned());
        self.wires.clear();
        self.hub.conns.clear();
        let real = self.actor(Who::Real);
        real.attempt = None;
        real.running = true;
        if real.stopped {
            self.violate(Violation::LegitimateLockedOut);
            return;
        }
        let mut quiet = NoChoice;
        for _ in 0..64 {
            if self.violation.is_some() {
                return;
            }
            let real = &self.actors[Who::Real.index()];
            if real.attempt.is_some_and(|attempt| attempt.in_session)
                && real.disk == self.hub.device.current
                && real.disk.is_some()
            {
                self.report.recoveries = self.report.recoveries.saturating_add(1);
                return;
            }
            let next = self
                .enabled()
                .into_iter()
                .map(|(event, _)| event)
                .find(|event| {
                    matches!(
                        event,
                        Event::Open(Who::Real)
                            | Event::WriteToDisk(Who::Real)
                            | Event::SendSaved(Who::Real)
                            | Event::ToHub(_)
                            | Event::ToConnector(_)
                            | Event::HubSendsCredential(_)
                            | Event::HubPromotes(_)
                    )
                });
            match next {
                Some(event) => self.apply(event, &mut quiet),
                None => break,
            }
        }
        if self.violation.is_none() {
            self.violate(Violation::LegitimateLockedOut);
        }
    }
}

/// The chooser of the quiet end: nothing is left to chance there.
struct NoChoice;

impl Chooser for NoChoice {
    fn below(&mut self, _bound: u64) -> u64 {
        // Never the value that makes a new connection slow.
        1
    }
}

const fn as_presented(presented: Option<Credential>) -> Presented {
    match presented {
        Some(credential) => Presented::Opened(credential),
        None => Presented::Nothing,
    }
}
