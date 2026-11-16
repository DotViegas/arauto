//! Rounds drawn from a seed for the waits of `await-reply` (decision 0048): register, answer in
//! the same thread, answer in another thread, a note from another peer, read, a dropped
//! connection, a restart of the leader, delays and timeouts, against the real hub and the fake
//! local API of Hermes.
//!
//! What must hold in every round:
//!
//! - **R1** at most one request to the local API for each idempotency key;
//! - **R2** every key is the identifier of a notice of the hub addressed to the asking Hermes;
//! - **R3** every announced arrival has a note from that peer to the asking Hermes in that
//!   thread;
//! - **R4** for each confirmed notice, the notices taken by a waiting process plus the normal
//!   wakes make exactly one; a late wake happens at most once, and only for a notice taken by a
//!   waiting process;
//! - **R5** a round with no wake mark toward the asking Hermes, and no note of another peer,
//!   makes no request to the local API;
//! - **R6** every wait ends by its minutes times the minute, plus 2 seconds;
//! - **R7** without a restart, an answer of the awaited peer during an armed wait ends that wait
//!   with an arrival (unless the answer was read at once).
//!
//! The seed is printed, and printed again with a failing round; `ARAUTO_ROUNDS_SEED` replays it.
//! A duplicate delivery of one notice is too slow to make against the real hub (its lease is a
//! minute): the unit rounds of the registry and `a_suppressed_notice_is_remembered_as_handled`
//! cover it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use arauto_connector::testing::Event;
use arauto_connector::{Awaited, await_reply};
use arauto_protocol::clock::SeededRng;
use arauto_testkit::hub::fixture::{Fixture, settings as hub_settings};
use uuid::Uuid;

use crate::replies::{Duo, notice_ids, settings_with};
use crate::support::{PATIENCE, connected, send};

/// How many rounds a run of the suite plays.
const ROUNDS: u64 = 6;

fn seed() -> u64 {
    std::env::var("ARAUTO_ROUNDS_SEED")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(0x5eed_a11e_2026)
}

/// One wait of the round.
struct Wait {
    started: Instant,
    minutes: u32,
    ended: Arc<Mutex<Option<Instant>>>,
    result: tokio::task::JoinHandle<Awaited>,
    dropped: bool,
}

#[derive(Default)]
struct Journal {
    steps: Vec<String>,
    /// Who wrote in which thread, as the test did it.
    writers: BTreeMap<Uuid, BTreeSet<String>>,
    replies: Vec<Instant>,
    reads: Vec<Instant>,
}

async fn round(fixture: &Fixture, seed: u64) -> Result<(), String> {
    let mut rng = SeededRng::new(seed);
    let mut journal = Journal::default();
    let wake_mark = rng.chance(2, 3);
    let poll = 150 + i64::try_from(rng.below(250)).unwrap_or(0);
    let minute = 1_500;
    let mut duo = Duo::new(fixture, wake_mark, (minute, poll, 1_000, 100)).await;
    journal
        .steps
        .push(format!("wake_mark={wake_mark} poll={poll}"));
    let (first, _) = duo.ask().await;
    journal
        .writers
        .entry(first)
        .or_default()
        .insert(duo.a_name.clone());
    let mut threads = vec![first];
    let mut waits: Vec<Wait> = Vec::new();
    let mut restarted = false;
    let mut third = None;
    let mut third_used = false;

    for _ in 0..(3 + rng.below(4)) {
        match rng.below(8) {
            // Register a wait on one of the threads.
            0 | 1 => {
                let thread = threads[usize::try_from(rng.below(threads.len() as u64)).unwrap_or(0)];
                let minutes = 1 + u32::try_from(rng.below(2)).unwrap_or(0);
                journal.steps.push(format!("register {thread} x{minutes}"));
                let ended = Arc::new(Mutex::new(None));
                let (home, settings) = (duo.a_folder.home.clone(), duo.client_settings());
                let marker = Arc::clone(&ended);
                let result = tokio::spawn(async move {
                    let awaited = await_reply(&home, thread, minutes, &settings).await;
                    *marker.lock().unwrap() = Some(Instant::now());
                    awaited
                });
                waits.push(Wait {
                    started: Instant::now(),
                    minutes,
                    ended,
                    result,
                    dropped: false,
                });
            }
            // An answer in the first thread, or in another one of the same peer.
            2 | 3 => {
                let thread = if rng.chance(1, 2) {
                    first
                } else {
                    if threads.len() == 1 {
                        let (second, _) = duo.ask().await;
                        journal
                            .writers
                            .entry(second)
                            .or_default()
                            .insert(duo.a_name.clone());
                        threads.push(second);
                    }
                    threads[1]
                };
                journal.steps.push(format!("reply {thread}"));
                duo.answer(thread, "an answer").await;
                journal
                    .writers
                    .entry(thread)
                    .or_default()
                    .insert(duo.b_name.clone());
                journal.replies.push(Instant::now());
            }
            // A note from a third Hermes.
            4 => {
                if third.is_none() {
                    let (settings, mut watch) = settings_with((minute, poll, 1_000, 100), None);
                    let (name, folder, connector) = connected(fixture, settings, &mut watch).await;
                    fixture
                        .admin
                        .link_add(&name, &duo.a_name, true, false, fixture.now())
                        .await
                        .map_err(|error| format!("{error:?}"))?;
                    third = Some((name, folder, connector));
                }
                if let Some((name, _, connector)) = &third {
                    journal.steps.push("note of a third Hermes".to_owned());
                    let sent = send(connector, &duo.a_name, "a note").await;
                    let thread = sent["thread"]
                        .as_str()
                        .and_then(arauto_connector::reply::parse_thread)
                        .ok_or("no thread")?;
                    journal
                        .writers
                        .entry(thread)
                        .or_default()
                        .insert(name.clone());
                    third_used = true;
                }
            }
            // The asking Hermes reads the first thread.
            5 => {
                journal.steps.push("read".to_owned());
                duo.read(first).await;
                journal.reads.push(Instant::now());
            }
            // A connection drops: one wait is given up.
            6 => {
                if let Some(wait) = waits.iter_mut().find(|wait| !wait.dropped) {
                    journal.steps.push("drop a wait".to_owned());
                    wait.result.abort();
                    wait.dropped = true;
                }
            }
            // The leader restarts.
            _ => {
                if !restarted && rng.chance(1, 2) {
                    journal.steps.push("restart".to_owned());
                    duo = duo.restarted().await;
                    restarted = true;
                }
            }
        }
        let delay = rng.below(300);
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }

    // Every wait ends (R6), and the grace checks run.
    let mut outcomes = Vec::new();
    for wait in waits {
        let limit = Duration::from_millis(u64::from(wait.minutes) * minute.unsigned_abs() + 2_000);
        if wait.dropped {
            continue;
        }
        let awaited = tokio::time::timeout(limit + PATIENCE, wait.result)
            .await
            .map_err(|_| format!("a wait did not end; steps {:?}", journal.steps))?
            .map_err(|error| error.to_string())?;
        let ended = wait.ended.lock().unwrap().unwrap_or_else(Instant::now);
        let took = ended.saturating_duration_since(wait.started);
        if took > limit {
            return Err(format!(
                "R6: a wait took {took:?}, limit {limit:?}; steps {:?}",
                journal.steps
            ));
        }
        outcomes.push((wait.started, wait.minutes, awaited));
    }
    tokio::time::sleep(Duration::from_millis(
        1_000 + 2 * poll.unsigned_abs() + 1_000,
    ))
    .await;

    // What the hub holds for the asking Hermes.
    let ids = notice_ids_of(fixture, &duo.a_name).await;
    let confirmed: BTreeSet<String> = ids
        .iter()
        .filter(|(_, state)| state == "confirmed")
        .map(|(id, _)| id.clone())
        .collect();
    let all_ids: BTreeSet<String> = ids.iter().map(|(id, _)| id.clone()).collect();
    let events = duo.events();
    let asked = duo.api.asked();
    let key_of = |head: &str| {
        head.lines()
            .find_map(|line| line.strip_prefix("Idempotency-Key: "))
            .map(str::to_owned)
            .unwrap_or_default()
    };
    let mut per_key: BTreeMap<String, usize> = BTreeMap::new();
    for (head, _) in &asked {
        *per_key.entry(key_of(head)).or_default() += 1;
    }
    let fail = |what: &str| Err(format!("{what}; steps {:?}", journal.steps));

    for (key, count) in &per_key {
        if *count > 1 {
            return fail(&format!("R1: {count} requests for the key {key}"));
        }
        if !all_ids.contains(key) {
            return fail(&format!("R2: the key {key} is no notice of the hub"));
        }
    }
    for (_, _, awaited) in &outcomes {
        if let Awaited::Arrived { thread, peer } = awaited {
            let wrote = journal
                .writers
                .get(thread)
                .is_some_and(|names| names.contains(peer.as_str()));
            if !wrote {
                return fail(&format!(
                    "R3: arrival in {thread} from {peer} without a note"
                ));
            }
        }
    }
    fn suppressed(event: &Event) -> Option<&String> {
        match event {
            Event::WakeSuppressed { notice } => Some(notice),
            _ => None,
        }
    }
    fn graced(event: &Event) -> Option<&String> {
        match event {
            Event::GraceWake { notice } => Some(notice),
            _ => None,
        }
    }
    let count_of = |wanted: fn(&Event) -> Option<&String>, id: &String| {
        events
            .iter()
            .filter_map(wanted)
            .filter(|notice| *notice == id)
            .count()
    };
    for id in &confirmed {
        let taken = count_of(suppressed, id);
        let late = count_of(graced, id);
        let requests = per_key.get(id).copied().unwrap_or(0);
        let normal = requests.saturating_sub(late);
        if taken + normal != 1 {
            return fail(&format!(
                "R4: notice {id}: {taken} taken, {normal} normal wakes"
            ));
        }
        if late > 1 || (late == 1 && taken == 0) {
            return fail(&format!(
                "R4: notice {id}: {late} late wakes, {taken} taken"
            ));
        }
    }
    if !wake_mark && !third_used && !asked.is_empty() {
        return fail(&format!("R5: {} requests without a wake mark", asked.len()));
    }
    if !restarted {
        for (started, minutes, awaited) in &outcomes {
            let deadline = *started + Duration::from_millis(u64::from(*minutes) * 1_500);
            for reply in &journal.replies {
                let armed = *reply >= *started + Duration::from_millis(700);
                let in_time = *reply + Duration::from_millis(1_000) < deadline;
                let read_soon = journal
                    .reads
                    .iter()
                    .any(|read| *read >= *reply && *read < *reply + Duration::from_millis(1_500));
                if armed && in_time && !read_soon && !matches!(awaited, Awaited::Arrived { .. }) {
                    return fail(&format!(
                        "R7: a reply during an armed wait ended as {awaited:?}"
                    ));
                }
            }
        }
    }
    Ok(())
}

async fn notice_ids_of(fixture: &Fixture, hermes: &str) -> Vec<(String, String)> {
    let _ = notice_ids;
    sqlx::query_as(
        "SELECT n.id::text, n.state FROM notice n JOIN hermes h ON h.id = n.hermes_id \
         WHERE h.name = $1 ORDER BY n.created_at, n.id",
    )
    .bind(hermes)
    .fetch_all(&fixture.pool)
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn rounds_of_waits_replies_drops_and_restarts_keep_one_announcement_per_reply() {
    let seed = seed();
    println!("reply rounds with seed {seed}; set ARAUTO_ROUNDS_SEED to replay");
    let fixture = Fixture::new(hub_settings()).await;
    let mut failures = Vec::new();
    for index in 0..ROUNDS {
        let round_seed = seed.wrapping_add(index);
        if let Err(why) = round(&fixture, round_seed).await {
            failures.push(format!("round {index} (seed {round_seed}): {why}"));
        }
    }
    assert!(failures.is_empty(), "seed {seed}: {failures:#?}");
}
