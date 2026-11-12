//! Rounds drawn from a seed, with the connector of the project against the hub running with the
//! test PostgreSQL (`docs/15-mvp.md`, section 7, M1): the connector fails at any point of the
//! swap, the hub drops the connection at any moment, sessions end abruptly, and in a third of
//! the rounds a copy of the files is taken and used. What must hold:
//!
//! - **without a copy**, the legitimate device never goes to quarantine and is never locked
//!   out: its next clean attempt reaches the session;
//! - **with a copy** that was used, the copy is found: once the copy and the true device have
//!   both spoken to the hub, the device is in quarantine.
//!
//! The seed is printed, and printed again with every failing round; `ARAUTO_ROUNDS_SEED`
//! replays it. The run fails if a kind of fault never happened.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use arauto_connector::Settings;
use arauto_connector::testing::{self, Attempted, Point};
use arauto_protocol::clock::SeededRng;
use arauto_protocol::errors::ErrorCode;
use arauto_testkit::hub::fixture::{Fixture, settings};
use futures_util::StreamExt;

use crate::support::{self, Folder, Once, approved, clean, retried};

/// How many rounds a run of the suite plays.
const ROUNDS: u64 = 120;

/// How many rounds run at the same moment, each with its own device.
const AT_ONCE: usize = 8;

fn seed() -> u64 {
    std::env::var("ARAUTO_ROUNDS_SEED")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(0x5eed_c0de_2026)
}

#[derive(Debug, Default)]
struct Played {
    events: Vec<String>,
}

impl Played {
    fn saw(&mut self, event: impl Into<String>) {
        self.events.push(event.into());
    }
}

/// One attempt of the true device with a fault drawn from `rng`.
async fn attempt(
    fixture: &Fixture,
    folder: &Folder,
    device: &str,
    settings: &Settings,
    rng: &mut SeededRng,
    played: &mut Played,
) -> Result<Attempted, String> {
    match rng.below(4) {
        // The connector fails at one point of the swap.
        0 => {
            let point = *rng.pick(&Point::SWAP).unwrap_or(&Point::BeforeSaved);
            played.saw(format!("connector fails {point:?}"));
            let faulty = settings.clone().with_faults(Once::at(point));
            testing::attempt(&folder.home, device, &faulty, Duration::ZERO)
                .await
                .map_err(|error| error.to_string())
        }
        // The hub drops the connection at a moment drawn between 0 and 60 ms.
        1 => {
            played.saw("hub drops");
            let wait = Duration::from_millis(rng.below(60));
            let identity = folder.public(device);
            let running = {
                let home = folder.home.clone();
                let device = device.to_owned();
                let settings = settings.clone();
                tokio::spawn(async move {
                    testing::attempt(&home, &device, &settings, Duration::from_millis(40)).await
                })
            };
            tokio::time::sleep(wait).await;
            for link in fixture.hub().links() {
                if link.info().identity == identity {
                    link.close_silently();
                }
            }
            match running.await {
                Ok(Ok(attempted)) => Ok(attempted),
                Ok(Err(error)) => Err(error.to_string()),
                Err(error) => Err(error.to_string()),
            }
        }
        // A clean attempt, whose session ends abruptly after a while.
        _ => {
            played.saw("session drops");
            let linger = Duration::from_millis(rng.below(40));
            testing::attempt(&folder.home, device, settings, linger)
                .await
                .map_err(|error| error.to_string())
        }
    }
}

async fn round(fixture: &Fixture, seed: u64) -> Result<Played, String> {
    let mut rng = SeededRng::new(seed);
    let mut played = Played::default();
    let (settings, _watch, _registrations) = support::settings();
    let settings = settings.with_seed(seed);
    let (_, folder) = approved(fixture, &settings).await;
    let device = folder.only_device();
    let identity = folder.public(&device);
    let attempts = rng.below(6);
    let copy_at = rng.chance(1, 3).then(|| rng.below(attempts + 1));
    let mut copy = None;
    for index in 0..=attempts {
        if copy_at == Some(index) {
            copy = Some(folder.copy());
        }
        if index == attempts {
            break;
        }
        let attempted =
            attempt(fixture, &folder, &device, &settings, &mut rng, &mut played).await?;
        if !retried(&attempted) {
            return Err(format!(
                "attempt {index} of the true device: {attempted:?}, after {:?}; the device is {:?}",
                played.events,
                fixture.state_of_key(&identity).await
            ));
        }
    }
    let state = fixture.state_of_key(&identity).await;
    if state.0 == "quarantined" {
        return Err(format!(
            "the true device went to quarantine alone: {state:?} after {:?}",
            played.events
        ));
    }
    match copy {
        None => {
            match clean(&folder, &device, &settings).await? {
                Attempted::Session(_) => played.saw("true device back in session"),
                other => return Err(format!("the true device was locked out: {other:?}")),
            }
            let state = fixture.state_of_key(&identity).await;
            if state.0 != "active" || fixture.alerts_of_key("quarantined", &identity).await != 0 {
                return Err(format!("after a clean attempt: {state:?}"));
            }
        }
        Some(copy) => {
            // The true device is off; the copy is used; then the true device comes back.
            match clean(&copy, &device, &settings).await? {
                Attempted::Closed(Some(ErrorCode::Ar501)) => {
                    played.saw("copy found when it was used")
                }
                Attempted::Session(_) => match clean(&folder, &device, &settings).await? {
                    Attempted::Closed(Some(ErrorCode::Ar501)) => {
                        played.saw("copy found when the true device came back");
                    }
                    other => return Err(format!("the true device came back to: {other:?}")),
                },
                other => return Err(format!("the copy: {other:?}")),
            }
            let state = fixture.state_of_key(&identity).await;
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
    println!("seed of the rounds: {seed}");
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
    let mut expected: Vec<String> = Point::SWAP
        .iter()
        .map(|point| format!("connector fails {point:?}"))
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
