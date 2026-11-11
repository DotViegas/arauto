//! Rounds drawn from a seed, over the network and against the hub running with the test
//! PostgreSQL (`docs/15-mvp.md`, section 7, M1): the connector drops at any step of the swap,
//! the hub drops the connection at any moment, sessions end abruptly, and sometimes a copy of
//! the files is taken and used. What must hold:
//!
//! - **without a copy**, the legitimate device never goes to quarantine and is never locked
//!   out: its next clean attempt always reaches the session;
//! - **with a copy** that was used, the copy is found: once the copy and the true device have
//!   both spoken to the hub, the device is in quarantine.
//!
//! The seed is printed when a round fails; `ARAUTO_ROUNDS_SEED` replays it. The run also
//! counts what it played, and fails if a kind of fault never happened: rounds that only ever
//! drew the easy path would prove nothing.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use arauto_protocol::clock::SeededRng;
use arauto_protocol::errors::ErrorCode;
use arauto_testkit::device::{Device, Faults, Reached, Step};
use futures_util::StreamExt;

use crate::fixture::{Fixture, settings};

/// How many rounds a run of the suite plays.
const ROUNDS: u64 = 120;

/// How many rounds run at the same moment, each with its own device.
const AT_ONCE: usize = 8;

fn seed() -> u64 {
    std::env::var("ARAUTO_ROUNDS_SEED")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(0x5eed_a7a0_2026)
}

/// Whether the connector tries again after this ending, without anything being wrong.
fn retried(reached: &Reached) -> bool {
    match reached {
        Reached::Closed { ended, .. } => matches!(
            ended.closing,
            None | Some(ErrorCode::Ar204 | ErrorCode::Ar205 | ErrorCode::Ar206 | ErrorCode::Ar208)
        ),
        Reached::Session(_) | Reached::Dropped(_) => false,
    }
}

/// What a round played, to count.
#[derive(Debug, Default)]
struct Played {
    events: Vec<String>,
}

impl Played {
    fn saw(&mut self, event: impl Into<String>) {
        self.events.push(event.into());
    }
}

/// One attempt with a fault drawn from `rng`.
async fn attempt(
    fixture: &Fixture,
    device: &Device,
    rng: &mut SeededRng,
    played: &mut Played,
) -> Result<Reached, String> {
    match rng.below(4) {
        // The connector drops at one of the steps of the swap.
        0 => {
            let step = *rng.pick(&Step::ALL).unwrap_or(&Step::BeforeSaved);
            played.saw(format!("connector drops {step:?}"));
            device
                .channel(Faults {
                    drop_at: Some(step),
                    ..Faults::default()
                })
                .await
                .map_err(|error| error.to_string())
        }
        // The hub drops the connection at a moment drawn between 0 and 60 ms.
        1 => {
            played.saw("hub drops");
            let wait = Duration::from_millis(rng.below(60));
            let running = {
                let device = device.clone();
                tokio::spawn(async move { device.channel(Faults::default()).await })
            };
            tokio::time::sleep(wait).await;
            for link in fixture.hub().links() {
                if link.info().identity == *device.public() {
                    link.close_silently();
                }
            }
            match running.await {
                Ok(Ok(reached)) => Ok(reached),
                // The network broke in the middle: the connector tries again.
                Ok(Err(_)) => Ok(Reached::Dropped(Step::BeforePresent)),
                Err(error) => Err(error.to_string()),
            }
        }
        // A clean attempt, whose session ends abruptly after a while.
        _ => {
            played.saw("session drops");
            let reached = device
                .channel(Faults::default())
                .await
                .map_err(|error| error.to_string())?;
            if let Reached::Session(mut session) = reached {
                let _ = session.idle(Duration::from_millis(rng.below(40))).await;
                return Ok(Reached::Dropped(Step::BeforeSession));
            }
            Ok(reached)
        }
    }
}

/// A clean attempt, tried again while the hub answers with what the connector retries.
async fn clean(device: &Device) -> Result<Reached, String> {
    for _ in 0..20 {
        let reached = device
            .channel(Faults::default())
            .await
            .map_err(|error| error.to_string())?;
        if retried(&reached) {
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        }
        return Ok(reached);
    }
    Err("no answer but retries after 20 clean attempts".to_owned())
}

async fn round(fixture: &Fixture, seed: u64) -> Result<Played, String> {
    let mut rng = SeededRng::new(seed);
    let mut played = Played::default();
    let (_, device) = fixture.approved().await;
    let attempts = rng.below(6);
    // A third of the rounds take a copy of the files at one of the attempts.
    let copy_at = rng.chance(1, 3).then(|| rng.below(attempts + 1));
    let mut copy = None;
    for index in 0..=attempts {
        if copy_at == Some(index) {
            copy = Some(device.copy_files());
        }
        if index == attempts {
            break;
        }
        let reached = attempt(fixture, &device, &mut rng, &mut played).await?;
        let harmless =
            matches!(reached, Reached::Session(_) | Reached::Dropped(_)) || retried(&reached);
        if !harmless {
            return Err(format!(
                "attempt {index} of the true device: {reached:?}, after {:?}; the device is {:?}",
                played.events,
                fixture.state_of(&device).await
            ));
        }
    }
    let state = fixture.state_of(&device).await;
    if state.0 == "quarantined" {
        return Err(format!(
            "the true device went to quarantine alone: {state:?}"
        ));
    }
    match copy {
        None => {
            // Never locked out: the next clean attempt reaches the session.
            match clean(&device).await? {
                Reached::Session(_) => played.saw("true device back in session"),
                other => return Err(format!("the true device was locked out: {other:?}")),
            }
            let state = fixture.state_of(&device).await;
            if state.0 != "active" || fixture.alerts("quarantined", &device).await != 0 {
                return Err(format!("after a clean attempt: {state:?}"));
            }
        }
        Some(copy) => {
            // The true device is off; the copy is used; then the true device comes back.
            match clean(&copy).await? {
                found if found.closing() == Some(ErrorCode::Ar501) => {
                    played.saw("copy found when it was used");
                }
                Reached::Session(_) => {
                    let back = clean(&device).await?;
                    if back.closing() != Some(ErrorCode::Ar501) {
                        return Err(format!("the true device came back to: {back:?}"));
                    }
                    played.saw("copy found when the true device came back");
                }
                other => return Err(format!("the copy: {other:?}")),
            }
            let state = fixture.state_of(&device).await;
            if state.0 != "quarantined" {
                return Err(format!("the copy was not found: {state:?}"));
            }
        }
    }
    Ok(played)
}

#[tokio::test(flavor = "multi_thread")]
async fn rounds_with_drops_on_both_sides_never_lock_out_the_device_and_find_the_copy() {
    let seed = seed();
    let mut chosen = settings();
    chosen.pool.size = 12;
    let fixture = Arc::new(Fixture::new(chosen).await);
    let outcomes: Vec<Result<Played, String>> = futures_util::stream::iter(0..ROUNDS)
        .map(|number| {
            let fixture = Arc::clone(&fixture);
            async move {
                let round_seed = seed.wrapping_add(number);
                round(&fixture, round_seed)
                    .await
                    .map_err(|failure| format!("round {number} (seed {round_seed}): {failure}"))
            }
        })
        .buffer_unordered(AT_ONCE)
        .collect()
        .await;
    let mut counted: BTreeMap<String, u32> = BTreeMap::new();
    let mut failures = Vec::new();
    for outcome in outcomes {
        match outcome {
            Ok(played) => {
                for event in played.events {
                    *counted.entry(event).or_default() += 1;
                }
            }
            Err(failure) => failures.push(failure),
        }
    }
    assert!(
        failures.is_empty(),
        "seed {seed}, {} of {ROUNDS} rounds failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    println!("seed {seed}: {counted:#?}");
    // Every kind of fault happened, and the copy was found both ways.
    let mut expected: Vec<String> = Step::ALL
        .iter()
        .map(|step| format!("connector drops {step:?}"))
        .collect();
    expected.extend(
        [
            "hub drops",
            "session drops",
            "true device back in session",
            "copy found when it was used",
            "copy found when the true device came back",
        ]
        .map(str::to_owned),
    );
    for event in expected {
        assert!(
            counted.get(&event).is_some_and(|count| *count > 0),
            "never played: {event}"
        );
    }
}
