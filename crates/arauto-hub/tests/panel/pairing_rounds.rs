//! The pairings of the panel under chance, in rounds drawn from a seed: double submits, a start
//! whose client gives up, polls from the session that started it and from another, a phone that
//! scans or not, slow and failing engine calls, cancels, and now and then a restart of the hub.
//!
//! `ARAUTO_TEST_SEED=<seed>` replays a run (a failing round prints its seed and its steps),
//! `ARAUTO_PAIRING_ROUNDS=<count>` runs more of them (24 by default).
//!
//! What every step must keep: no QR or code in an answer that is not the starter's window of a
//! live run; at most three numbers in pairing (a restart leaves orphans, counted apart); two
//! starts of one number from one session lead to one run; a read never moves a deadline; and
//! with no fault, no cancel and no restart, a pairing is not ended before its deadline by
//! another number's pairing. At the settle (everything healed, the deadline passed, the
//! supervision run) the row agrees with the engine.

use std::collections::HashMap;
use std::time::Duration;

use arauto_hub::core::number::NumberState;
use arauto_protocol::clock::SeededRng;
use arauto_testkit::engine::Fault;

use crate::Browser;
use crate::numbers::{Rig, rig_with};
use crate::world::phone;

const DEFAULT_SEED: u64 = 0x0050_4149_5249_4e47;
const DEADLINE_MS: i64 = 1_500;

fn number_from(variable: &str) -> Option<u64> {
    let text = std::env::var(variable).ok()?;
    Some(
        text.trim()
            .parse()
            .unwrap_or_else(|_| panic!("{variable} must be a number, and it is {text:?}")),
    )
}

#[derive(Default)]
struct Round {
    steps: Vec<String>,
    faulted: bool,
    stop_faulted: bool,
    cancelled: bool,
    restarted: bool,
    /// The deadline of each number whose pairing this round started.
    until: HashMap<String, i64>,
    /// The run each start of this session led to.
    runs: HashMap<String, String>,
    /// Each number's phone, and whether the scan set was the expected one.
    scanned: HashMap<String, Option<bool>>,
}

fn holds_a_secret(page: &str) -> bool {
    page.contains("<svg class=\"qr\"") || page.contains("ABCD-EFGH")
}

async fn state_of(rig: &Rig, alias: &str) -> (NumberState, Option<i64>) {
    let number = rig.world.number(alias).await;
    (number.state, number.pairing_until)
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread")]
async fn pairing_rounds_hold_their_rules() {
    let seed = number_from("ARAUTO_TEST_SEED").unwrap_or(DEFAULT_SEED);
    let rounds = number_from("ARAUTO_PAIRING_ROUNDS").unwrap_or(24);
    println!("pairing rounds: seed {seed}, {rounds} rounds (ARAUTO_TEST_SEED replays)");
    let mut rig = rig_with(DEADLINE_MS).await;
    // The first number of the organization asks for the acceptance.
    rig.add("seed-number", &phone(5)).await;
    let mut s1 = rig.cookie.clone();
    let mut s2 = rig.other_session().await;

    for round in 0..rounds {
        let mut rng = SeededRng::new(seed ^ round.wrapping_mul(0x9E37_79B9));
        let mut log = Round::default();
        let result = std::panic::AssertUnwindSafe(play(
            &mut rig, &mut s1, &mut s2, &mut rng, round, &mut log,
        ));
        // A failing round says how to replay it.
        if let Err(panic) = futures_util::FutureExt::catch_unwind(result).await {
            eprintln!(
                "round {round} failed: ARAUTO_TEST_SEED={seed}; steps: {:?}",
                log.steps
            );
            std::panic::resume_unwind(panic);
        }
    }
}

async fn play(
    rig: &mut Rig,
    s1: &mut String,
    s2: &mut String,
    rng: &mut SeededRng,
    round: u64,
    log: &mut Round,
) {
    let base = u32::try_from(200 + round * 2).unwrap();
    let aliases = [format!("r{round}a"), format!("r{round}b")];
    for (index, alias) in aliases.iter().enumerate() {
        rig.add(alias, &phone(base + u32::try_from(index).unwrap()))
            .await;
    }
    let sessions: Vec<String> = {
        let mut found = Vec::new();
        for alias in &aliases {
            found.push(rig.world.number(alias).await.session.as_str().to_owned());
        }
        found
    };
    let restart_round = rng.below(6) == 0;
    let steps = 6 + rng.below(5);
    let mut restarted_at = None;
    if restart_round {
        restarted_at = Some(rng.below(steps));
    }
    for step in 0..steps {
        let which = usize::try_from(rng.below(2)).unwrap();
        let alias = aliases[which].clone();
        let kind = rng.below(13);
        if restarted_at == Some(step) {
            log.steps.push("restart the hub".into());
            rig.world.fixture.stop().await;
            rig.world.fixture.start().await;
            rig.browser = Browser::of(&rig.world.fixture);
            for cookie in [&mut *s1, &mut *s2] {
                let answer = rig.browser.pass(&mut rig.authenticator).await;
                assert_eq!(answer.status, 303, "{}", answer.text());
                *cookie = crate::cookie_of(&answer).unwrap();
            }
            rig.cookie = s1.clone();
            log.restarted = true;
            // The runs went with the process.
            log.runs.clear();
            continue;
        }
        match kind {
            0 | 1 => {
                let method = if kind == 0 { "qr" } else { "code" };
                log.steps.push(format!("start {method} {alias}"));
                let answer = rig.start_with(s1, &alias, method).await;
                if answer.status == 303 {
                    let run = answer
                        .header("location")
                        .and_then(|l| l.split("run=").nth(1))
                        .unwrap()
                        .to_owned();
                    log.runs.insert(alias.clone(), run);
                    let (_, until) = state_of(rig, &alias).await;
                    if let Some(until) = until {
                        log.until.entry(alias.clone()).or_insert(until);
                    }
                }
            }
            2 => {
                log.steps.push(format!("double submit {alias}"));
                let (one, two) = tokio::join!(
                    rig.start_with(s1, &alias, "qr"),
                    rig.start_with(s1, &alias, "qr")
                );
                // With the engine failing, the first run may end before the second start.
                if one.status == 303 && two.status == 303 && !log.faulted {
                    assert_eq!(
                        one.header("location"),
                        two.header("location"),
                        "two starts of one number from one session lead to one run"
                    );
                }
                if let Some(location) = one.header("location").or(two.header("location")) {
                    log.runs.insert(
                        alias.clone(),
                        location.split("run=").nth(1).unwrap().to_owned(),
                    );
                }
            }
            3 => {
                log.steps.push(format!("start that gives up {alias}"));
                let body = format!("alias={alias}&method=qr");
                let call = rig.browser.post("/whatsapps/pair/start", Some(s1), &body);
                let _ = tokio::time::timeout(Duration::from_millis(30), call).await;
            }
            4..=6 => {
                let (cookie, with_run, who) = match kind {
                    4 => (s1.clone(), true, "s1 with the run"),
                    5 => (s1.clone(), false, "s1 with no run"),
                    _ => (s2.clone(), true, "s2 with the run"),
                };
                log.steps.push(format!("poll {who} {alias}"));
                let path = match (with_run, log.runs.get(&alias)) {
                    (true, Some(run)) => Rig::wait_path(&alias, run),
                    _ => format!("/whatsapps/pair/wait?alias={alias}"),
                };
                let before = state_of(rig, &alias).await;
                let answer = rig.browser.get(&path, Some(&cookie)).await;
                let page = answer.text();
                if holds_a_secret(&page) {
                    assert!(who == "s1 with the run", "{who} was shown a QR or a code");
                    assert!(
                        answer
                            .header("cache-control")
                            .is_some_and(|v| v.contains("no-store"))
                    );
                }
                let after = state_of(rig, &alias).await;
                if before.0 == NumberState::Pairing && after.0 == NumberState::Pairing {
                    assert_eq!(before.1, after.1, "a read moved the deadline");
                }
            }
            7 => {
                let expected = rng.below(2) == 0;
                if log.scanned.contains_key(&alias) {
                    log.steps.push(format!("scan {alias} (already set)"));
                    continue;
                }
                log.steps.push(format!("scan {alias} expected={expected}"));
                let own = phone(base + u32::try_from(which).unwrap());
                let given = if expected { own } else { phone(base + 100) };
                rig.world.fake.pairs_as(&sessions[which], &given);
                log.scanned.insert(alias.clone(), Some(expected));
            }
            8 => {
                let (needle, fault) = match rng.below(3) {
                    0 => ("/auth/qr", Fault::Slow(Duration::from_millis(300))),
                    1 => ("/sessions/", Fault::ServerError),
                    _ => ("/stop", Fault::ServerError),
                };
                log.steps.push(format!("fault {needle}"));
                rig.world.fake.fail(needle, fault);
                log.faulted = true;
                log.stop_faulted |= needle != "/auth/qr";
            }
            9 => {
                log.steps.push("heal".into());
                rig.world.fake.heal();
            }
            10 => {
                let (cookie, who) = if rng.below(2) == 0 {
                    (s1.clone(), "s1")
                } else {
                    (s2.clone(), "s2")
                };
                log.steps.push(format!("cancel {who} {alias}"));
                let run = log.runs.get(&alias).cloned().unwrap_or_default();
                let answer = rig
                    .browser
                    .post(
                        "/whatsapps/pair/cancel",
                        Some(&cookie),
                        &format!("alias={alias}&run={run}"),
                    )
                    .await;
                assert_eq!(answer.status, 303);
                log.cancelled = true;
            }
            _ => {
                let wait = rng.below(400);
                log.steps.push(format!("wait {wait} ms"));
                tokio::time::sleep(Duration::from_millis(wait)).await;
            }
        }
        // After every step: the ceilings, and no pairing ended early by what it did not cause.
        let pairing: i64 =
            sqlx::query_scalar("SELECT count(*) FROM number WHERE state = 'pairing'")
                .fetch_one(&rig.world.fixture.pool)
                .await
                .unwrap();
        assert!(
            pairing <= if log.restarted { 6 } else { 3 },
            "{pairing} numbers in pairing at once"
        );
        if !(log.faulted || log.cancelled || log.restarted) {
            for (alias, until) in &log.until {
                let (state, _) = state_of(rig, alias).await;
                if state == NumberState::Unpaired {
                    assert!(
                        rig.world.now() >= *until - 300,
                        "the pairing of {alias} ended before its deadline"
                    );
                }
            }
        }
    }

    // The settle: everything healed, the deadline passed, the supervision run.
    rig.world.fake.heal();
    tokio::time::sleep(Duration::from_millis(
        u64::try_from(DEADLINE_MS).unwrap() + 800,
    ))
    .await;
    arauto_hub::engine::supervise(rig.world.hub()).await;
    for (index, alias) in aliases.iter().enumerate() {
        let status = rig.world.fake.status(&sessions[index]);
        let state = rig.world.number(alias).await.state;
        match (status.as_deref(), log.scanned.get(alias).copied().flatten()) {
            // A hub that restarted lost the loop that watched this session: the row is what the
            // supervision made of it, and no more is asked of it here.
            (Some("WORKING"), _) if log.restarted => {}
            (Some("WORKING"), Some(true)) => {
                assert_eq!(state, NumberState::Connected, "{alias} scanned as expected");
            }
            (Some("WORKING"), Some(false)) => {
                assert_eq!(
                    state,
                    NumberState::NumberSwapped,
                    "{alias} scanned as another phone"
                );
            }
            (Some("WORKING"), None) => panic!("{alias} works and nobody scanned it"),
            (status, _) => {
                assert_eq!(
                    state,
                    NumberState::Unpaired,
                    "{alias} was not scanned: {status:?}"
                );
                // A stop that failed on purpose is not tried again, and a restarted hub leaves
                // the session waiting for a QR: both are said in the documents.
                if !(log.stop_faulted || log.restarted) {
                    assert_eq!(status, Some("STOPPED"), "{alias} left its session running");
                }
            }
        }
    }
}
