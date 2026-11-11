//! The credential swap under random rounds: stops of both sides, delays, late messages of
//! abandoned attempts, repetitions, the 24-hour swap and a copy of the files of the connector.
//!
//! The rounds run in memory, on the simulator of `arauto_hub::core::swap_model`, which decides
//! with the same pure functions the hub uses. A failing round prints its seed and every step;
//! `ARAUTO_SWAP_ROUND=<seed>` replays that one round, `ARAUTO_TEST_SEED=<seed>` moves the
//! whole run to other rounds, and `ARAUTO_SWAP_ROUNDS=<count>` runs more of them.
//!
//! After the rounds with the documented rules, one test per invariant runs a hub with one
//! rule removed and shows that the simulator catches it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use arauto_hub::core::judge::{
    self, Credential, Decision, DeviceSnapshot, Mode, Presented, QuarantineReason, Transition,
    TransitionError,
};
use arauto_hub::core::model::DeviceState;
use arauto_hub::core::swap_model::{
    Chooser, Documented, Failure, Invariant, Report, Rules, Violation, run_round,
};
use arauto_protocol::clock::SeededRng;
use arauto_protocol::message::Generation;

/// How many rounds the documented rules go through, at least.
const ROUNDS: u64 = 50_000;

/// The seed of the run when `ARAUTO_TEST_SEED` does not give another.
const DEFAULT_SEED: u64 = 0x4152_4155_544f;

/// How many rounds a wrong hub is given to be caught.
const MUTANT_ROUNDS: u64 = 100_000;

struct Seeded(SeededRng);

impl Chooser for Seeded {
    fn below(&mut self, bound: u64) -> u64 {
        self.0.below(bound)
    }
}

fn number_from(variable: &str) -> Option<u64> {
    let text = std::env::var(variable).ok()?;
    Some(
        text.trim()
            .parse()
            .unwrap_or_else(|_| panic!("{variable} must be a number, and it is {text:?}")),
    )
}

fn base_seed() -> u64 {
    number_from("ARAUTO_TEST_SEED").unwrap_or(DEFAULT_SEED)
}

fn round(rules: &(impl Rules + ?Sized), seed: u64) -> Result<Report, Failure> {
    run_round(rules, &mut Seeded(SeededRng::new(seed)))
}

#[test]
fn fifty_thousand_random_rounds_keep_every_invariant() {
    if let Some(seed) = number_from("ARAUTO_SWAP_ROUND") {
        match round(&Documented, seed) {
            Ok(report) => println!("round {seed} keeps every invariant: {report:#?}"),
            Err(failure) => panic!("round {seed} breaks an invariant.\n{failure}"),
        }
        return;
    }
    let base = base_seed();
    let rounds = number_from("ARAUTO_SWAP_ROUNDS").map_or(ROUNDS, |asked| asked.max(ROUNDS));
    let workers = std::thread::available_parallelism().map_or(4, |count| count.get() as u64);
    let outcomes: Vec<Result<Report, (u64, Failure)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                scope.spawn(move || {
                    let mut total = Report::default();
                    // Every worker takes the rounds whose number leaves its remainder.
                    for number in (worker..rounds).step_by(workers as usize) {
                        let seed = base.wrapping_add(number);
                        match round(&Documented, seed) {
                            Ok(report) => total.add(&report),
                            Err(failure) => return Err((seed, failure)),
                        }
                    }
                    Ok(total)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a worker does not panic"))
            .collect()
    });

    let mut total = Report::default();
    for outcome in outcomes {
        match outcome {
            Ok(report) => total.add(&report),
            Err((seed, failure)) => panic!(
                "the round of seed {seed} breaks an invariant; replay it with \
                 ARAUTO_SWAP_ROUND={seed}\n{failure}"
            ),
        }
    }
    println!("{rounds} rounds from seed {base} (ARAUTO_TEST_SEED picks another): {total:#?}");

    // The rounds must reach what they are meant to try. A change in the simulator that
    // stopped drawing one of these would otherwise pass in silence.
    let reached: &[(&str, u64, u64)] = &[
        ("sessions", total.sessions, ROUNDS),
        ("24-hour swaps", total.rotations, 1_000),
        (
            "pending credentials promoted by being presented",
            total.promotions_by_presentation,
            1_000,
        ),
        (
            "pending credentials discarded",
            total.pending_discarded,
            1_000,
        ),
        (
            "rounds with the copy in use",
            total.rounds_with_copy_in_use,
            10_000,
        ),
        ("quarantines for reuse", total.quarantines, 5_000),
        ("double connections", total.double_connections, 200),
        (
            "attempts that were not the newest",
            total.not_the_newest,
            200,
        ),
        ("older attempts closed", total.older_closed, 1_000),
        (
            "connections given up for dead",
            total.given_up_for_dead,
            1_000,
        ),
        ("hub stops", total.hub_stops, 10_000),
        (
            "hub stops between writing the pending credential and sending it",
            total.hub_stops_between_write_and_send,
            100,
        ),
        (
            "hub stops between hearing the saved and writing the promotion",
            total.hub_stops_between_saved_and_promotion,
            100,
        ),
        (
            "connector stops before writing the credential",
            total.connector_stops_before_writing,
            100,
        ),
        (
            "connector stops between writing and saying saved",
            total.connector_stops_before_saved,
            100,
        ),
        (
            "late messages of abandoned attempts",
            total.late_messages,
            1_000,
        ),
        ("late check_ok", total.late_check_ok, 100),
        ("messages delivered twice", total.duplicates, 10_000),
        (
            "rounds that ended with the lone device in session",
            total.recoveries,
            20_000,
        ),
    ];
    for (what, count, at_least) in reached {
        assert!(
            count >= at_least,
            "the rounds reached {what} only {count} times (at least {at_least} expected)"
        );
    }
}

#[test]
fn a_round_is_the_same_for_the_same_seed() {
    let base = base_seed();
    for number in 0..50 {
        let seed = base.wrapping_add(number);
        assert_eq!(
            round(&Documented, seed),
            round(&Documented, seed),
            "seed {seed}"
        );
    }
    // And different seeds give different rounds.
    let reports: Vec<_> = (0..50)
        .map(|number| round(&Documented, base.wrapping_add(number)))
        .collect();
    assert!(reports.windows(2).any(|pair| pair[0] != pair[1]));
}

// ---------------------------------------------------------------------------- wrong hubs

/// Runs a wrong hub until the simulator catches it breaking `wanted`, and returns the seed
/// and the failure. The same seed must pass with the documented rules: the only difference
/// between the two runs is the rule that was removed.
fn caught(rules: &impl Rules, wanted: Invariant) -> (u64, Failure) {
    let base = base_seed();
    let mut others = std::collections::BTreeMap::new();
    for number in 0..MUTANT_ROUNDS {
        let seed = base.wrapping_add(number);
        match round(rules, seed) {
            Ok(_) => {}
            Err(failure) if failure.violation.invariant() == wanted => {
                assert!(
                    !failure.trace.is_empty(),
                    "the failure of seed {seed} has its steps"
                );
                println!(
                    "caught at seed {seed}, after {} rounds: {:?}",
                    number + 1,
                    failure.violation
                );
                if let Err(documented) = round(&Documented, seed) {
                    panic!("the documented rules fail on seed {seed} too.\n{documented}");
                }
                return (seed, failure);
            }
            Err(failure) => {
                *others.entry(failure.violation.invariant()).or_insert(0u64) += 1;
            }
        }
    }
    panic!(
        "{MUTANT_ROUNDS} rounds did not catch this hub breaking {wanted:?}; what they caught: \
         {others:?}"
    );
}

/// Without "only the newest connection is judged" (steps 5 and 6): an attempt the connector
/// already abandoned is judged when its `check_ok` arrives late.
struct JudgesAbandonedAttempts;

impl Rules for JudgesAbandonedAttempts {
    fn judges_only_the_newest(&self) -> bool {
        false
    }
}

#[test]
fn invariant_a_judging_an_abandoned_attempt_quarantines_a_legitimate_device() {
    let (_, failure) = caught(
        &JudgesAbandonedAttempts,
        Invariant::LegitimateDeviceKeepsWorking,
    );
    assert_eq!(failure.violation, Violation::LegitimateQuarantined);
}

/// Without the second phase of the swap: the pending credential is not accepted when it is
/// presented. A connector that wrote the new credential and lost the connection before the
/// hub heard its "saved" comes back with a credential the hub refuses.
struct IgnoresThePendingOne;

impl Rules for IgnoresThePendingOne {
    fn judge(&self, device: &DeviceSnapshot, presented: &Presented, mode: Mode) -> Decision {
        match judge::judge(device, presented, mode) {
            Decision::Accept {
                promote_pending: true,
            } => Decision::Nothing,
            decision => decision,
        }
    }
}

#[test]
fn invariant_a_a_drop_in_the_middle_of_the_swap_must_not_lock_the_device_out() {
    let (_, failure) = caught(
        &IgnoresThePendingOne,
        Invariant::LegitimateDeviceKeepsWorking,
    );
    assert_eq!(failure.violation, Violation::LegitimateLockedOut);
}

/// Without "another one already issued is reuse": any seal that opens is accepted.
struct AcceptsAnySealThatOpens;

impl Rules for AcceptsAnySealThatOpens {
    fn judge(&self, device: &DeviceSnapshot, presented: &Presented, mode: Mode) -> Decision {
        match (judge::judge(device, presented, mode), presented) {
            (Decision::Quarantine(QuarantineReason::Reuse), Presented::Opened(_)) => {
                Decision::Accept {
                    promote_pending: false,
                }
            }
            (decision, _) => decision,
        }
    }
}

#[test]
fn invariant_b_a_swapped_credential_accepted_is_caught() {
    let (_, failure) = caught(
        &AcceptsAnySealThatOpens,
        Invariant::SwappedCredentialNeverAccepted,
    );
    assert_eq!(failure.violation, Violation::SwappedCredentialAccepted);
}

/// Without rule 8 of C7: the identity alone is accepted after generation zero.
struct AcceptsTheIdentityAlone;

impl Rules for AcceptsTheIdentityAlone {
    fn judge(&self, device: &DeviceSnapshot, presented: &Presented, mode: Mode) -> Decision {
        match (judge::judge(device, presented, mode), presented) {
            (Decision::Quarantine(QuarantineReason::Reuse), Presented::Nothing) => {
                Decision::Accept {
                    promote_pending: false,
                }
            }
            (decision, _) => decision,
        }
    }
}

#[test]
fn invariant_b_the_identity_alone_accepted_past_generation_zero_is_caught() {
    let (_, failure) = caught(
        &AcceptsTheIdentityAlone,
        Invariant::SwappedCredentialNeverAccepted,
    );
    assert_eq!(
        failure.violation,
        Violation::NothingAcceptedPastGenerationZero
    );
}

/// Without rule 4 of C7: the session starts when the credential is issued, before the
/// "saved". Whoever holds a copy uses the hub without ever confirming, so the credential of
/// the real device never dies.
struct ServesBeforeTheSaved;

impl Rules for ServesBeforeTheSaved {
    fn serves_only_after_saved(&self) -> bool {
        false
    }
}

#[test]
fn invariant_c_serving_before_the_saved_lets_a_copy_go_unnoticed() {
    let (_, failure) = caught(&ServesBeforeTheSaved, Invariant::CopyNeverSilent);
    assert_eq!(failure.violation, Violation::SilentUseOfBoth);
}

/// Without "the largest generation ever issued": the next credential is numbered from the
/// current one, so a number given to a credential that was discarded is given again.
struct NumbersFromTheCurrentOne;

impl Rules for NumbersFromTheCurrentOne {
    fn next_generation(&self, device: &DeviceSnapshot) -> Result<Generation, TransitionError> {
        let current = device.current.map_or(0, |current| current.generation.get());
        Generation::new(current + 1).ok_or(TransitionError::GenerationExhausted)
    }

    fn issue(
        &self,
        device: &DeviceSnapshot,
        promote_pending: bool,
        new: Credential,
    ) -> Result<Transition, TransitionError> {
        // `judge::issue` without the check that the generation is new.
        let mut after = *device;
        let mut first_confirmation = false;
        if promote_pending {
            let promoted = device.pending.ok_or(TransitionError::NoPending)?;
            first_confirmation = device.state == DeviceState::Approved;
            after.current = Some(promoted);
            after.state = DeviceState::Active;
        }
        after.pending = Some(new);
        after.max_generation = after.max_generation.max(new.generation.get());
        Ok(Transition {
            device: after,
            first_confirmation,
        })
    }
}

#[test]
fn invariant_d_a_generation_given_twice_is_caught() {
    let (_, failure) = caught(&NumbersFromTheCurrentOne, Invariant::OneLineOfCredentials);
    assert_eq!(failure.violation, Violation::GenerationNotNew);
}

/// Without the probe of step 8: a presentation that would be accepted goes ahead while
/// another connection of the device is being served.
struct NeverProbesTheEstablished;

impl Rules for NeverProbesTheEstablished {
    fn probes_the_established(&self) -> bool {
        false
    }
}

#[test]
fn invariant_d_two_sessions_at_once_are_caught() {
    let (_, failure) = caught(&NeverProbesTheEstablished, Invariant::OneLineOfCredentials);
    assert_eq!(failure.violation, Violation::TwoSessions);
}

/// Without rule 7 of C7: any "saved" promotes whatever this connection received, even when
/// it is no longer the pending credential.
struct PromotesWhateverWasSaved;

impl Rules for PromotesWhateverWasSaved {
    fn confirm(
        &self,
        device: &DeviceSnapshot,
        issued_here: &Credential,
        _saved: &Credential,
    ) -> Result<Transition, TransitionError> {
        let mut after = *device;
        after.current = Some(*issued_here);
        after.pending = None;
        after.state = DeviceState::Active;
        Ok(Transition {
            device: after,
            first_confirmation: device.state == DeviceState::Approved,
        })
    }
}

#[test]
fn invariant_a_a_saved_about_another_credential_must_not_promote() {
    // A "saved" that arrives twice is heard again after the 24-hour swap issued the next
    // credential. Promoting on it kills the credential the connector really has.
    let (_, failure) = caught(
        &PromotesWhateverWasSaved,
        Invariant::LegitimateDeviceKeepsWorking,
    );
    assert_eq!(failure.violation, Violation::LegitimateQuarantined);
}
