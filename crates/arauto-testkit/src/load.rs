//! A generator of load (`docs/15-mvp.md`, section 3): many devices, each with its own
//! identity, coming back to the hub at once, keeping the heartbeat, and reading locally.
//!
//! - [`come_back`] brings every device to its session at the same moment. A refusal the
//!   connector retries (429, 503, AR-204, AR-205, AR-206, AR-208, a dropped connection) is
//!   tried again after a random wait whose ceiling doubles at each failure, as the connector
//!   does; anything else (quarantine, revocation) is a failure of the round, and is reported.
//! - [`local_reads`] has every session call `arauto_status`, the tool the hub answers without
//!   the engine, at a fixed rhythm, and measures each answer.
//!
//! The random waits come from a seed, so that a round can be repeated.

use std::time::Duration;

use arauto_protocol::clock::SeededRng;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::CallOutcome;
use arauto_protocol::tools::ARAUTO_STATUS;
use serde_json::json;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::KitError;
use crate::device::{Device, Faults, Reached, SessionClient};

/// The waits between two attempts of one device: the first one is drawn below `first_ms`,
/// and the ceiling doubles up to `max_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    /// The ceiling of the first wait.
    pub first_ms: u64,
    /// The largest ceiling.
    pub max_ms: u64,
}

impl Backoff {
    /// The waits of the connector: from 1 to 60 seconds (`docs/14-protocolo-do-canal.md`,
    /// section 4).
    pub const CONNECTOR: Backoff = Backoff {
        first_ms: 1_000,
        max_ms: 60_000,
    };

    fn wait(self, failures: u32, rng: &mut SeededRng) -> Duration {
        let ceiling = self
            .first_ms
            .saturating_mul(1u64 << failures.min(20))
            .min(self.max_ms)
            .max(1);
        Duration::from_millis(rng.below(ceiling))
    }
}

/// What [`come_back`] did.
#[derive(Debug)]
pub struct Comeback {
    /// The sessions, one per device that made it.
    pub sessions: Vec<SessionClient>,
    /// From the start to the last session.
    pub elapsed: Duration,
    /// How many attempts were refused and tried again.
    pub retries: u64,
    /// What went wrong for each device that did not make it.
    pub failures: Vec<String>,
}

/// Whether the connector tries again after this.
fn retried(error: &KitError) -> bool {
    matches!(
        error,
        KitError::Refused(429 | 503)
            | KitError::TimedOut
            | KitError::Io(_)
            | KitError::Ws(_)
            | KitError::Ended(_)
    )
}

fn retried_code(code: Option<ErrorCode>) -> bool {
    matches!(
        code,
        None | Some(ErrorCode::Ar204 | ErrorCode::Ar205 | ErrorCode::Ar206 | ErrorCode::Ar208)
    )
}

/// Brings every device to its session at once, within `deadline`.
pub async fn come_back(
    devices: Vec<Device>,
    deadline: Duration,
    backoff: Backoff,
    seed: u64,
) -> Comeback {
    let started = Instant::now();
    let until = started + deadline;
    let mut running = JoinSet::new();
    for (index, device) in devices.into_iter().enumerate() {
        let seed = seed.wrapping_add(u64::try_from(index).unwrap_or(u64::MAX));
        running.spawn(async move {
            let mut rng = SeededRng::new(seed);
            let mut failures = 0u32;
            let mut retries = 0u64;
            loop {
                if Instant::now() >= until {
                    return (Err("no session before the deadline".to_owned()), retries);
                }
                match device.channel(Faults::default()).await {
                    Ok(Reached::Session(session)) => return (Ok(*session), retries),
                    Ok(Reached::Closed { ended, state }) if retried_code(ended.closing) => {
                        let _ = state;
                    }
                    Ok(other) => return (Err(format!("{other:?}")), retries),
                    Err(error) if retried(&error) => {}
                    Err(error) => return (Err(error.to_string()), retries),
                }
                retries = retries.saturating_add(1);
                let wait = backoff.wait(failures, &mut rng);
                failures = failures.saturating_add(1);
                tokio::time::sleep(wait).await;
            }
        });
    }
    let mut comeback = Comeback {
        sessions: Vec::new(),
        elapsed: Duration::ZERO,
        retries: 0,
        failures: Vec::new(),
    };
    while let Some(done) = running.join_next().await {
        match done {
            Ok((outcome, retries)) => {
                comeback.retries = comeback.retries.saturating_add(retries);
                match outcome {
                    Ok(session) => comeback.sessions.push(session),
                    Err(failure) => comeback.failures.push(failure),
                }
            }
            Err(error) => comeback.failures.push(error.to_string()),
        }
    }
    comeback.elapsed = started.elapsed();
    comeback
}

/// The distribution of a set of measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Latencies {
    /// How many measures.
    pub count: usize,
    /// Half of them took at most this.
    pub p50: Duration,
    /// 95% of them took at most this.
    pub p95: Duration,
    /// 99% of them took at most this.
    pub p99: Duration,
    /// The longest.
    pub max: Duration,
}

impl Latencies {
    /// The distribution of `samples`.
    #[must_use]
    pub fn of(mut samples: Vec<Duration>) -> Self {
        samples.sort_unstable();
        let at = |share: usize| -> Duration {
            if samples.is_empty() {
                return Duration::ZERO;
            }
            // The smallest measure with at least `share` percent of them at or below it.
            let rank = samples.len().saturating_mul(share).div_ceil(100).max(1);
            samples
                .get(rank.saturating_sub(1))
                .copied()
                .unwrap_or_default()
        };
        Self {
            count: samples.len(),
            p50: at(50),
            p95: at(95),
            p99: at(99),
            max: samples.last().copied().unwrap_or_default(),
        }
    }
}

/// What [`local_reads`] measured.
#[derive(Debug)]
pub struct Reads {
    /// The sessions, given back.
    pub sessions: Vec<SessionClient>,
    /// The time of each `arauto_status`, from the call to the result.
    pub latencies: Latencies,
    /// What went wrong, per session.
    pub failures: Vec<String>,
}

/// Every session calls `arauto_status` `rounds` times, one every `every`, starting at a random
/// moment of the first interval; between calls it keeps the heartbeat and the swaps.
pub async fn local_reads(
    sessions: Vec<SessionClient>,
    rounds: usize,
    every: Duration,
    seed: u64,
) -> Reads {
    let mut running = JoinSet::new();
    for (index, mut session) in sessions.into_iter().enumerate() {
        let seed = seed.wrapping_add(u64::try_from(index).unwrap_or(u64::MAX));
        running.spawn(async move {
            let mut rng = SeededRng::new(seed);
            let every_ms = u64::try_from(every.as_millis()).unwrap_or(u64::MAX).max(1);
            let mut samples = Vec::with_capacity(rounds);
            let start = Duration::from_millis(rng.below(every_ms));
            if let Err(error) = session.idle(start).await {
                return (session, samples, Some(error.to_string()));
            }
            for _ in 0..rounds {
                let asked = Instant::now();
                match session.call(ARAUTO_STATUS, json!({})).await {
                    Ok(CallOutcome::Ok(_)) => samples.push(asked.elapsed()),
                    Ok(CallOutcome::Refused { code, .. }) => {
                        return (session, samples, Some(format!("refused with {code}")));
                    }
                    Err(error) => return (session, samples, Some(error.to_string())),
                }
                let left = every.saturating_sub(asked.elapsed());
                if let Err(error) = session.idle(left).await {
                    return (session, samples, Some(error.to_string()));
                }
            }
            (session, samples, None)
        });
    }
    let mut sessions = Vec::new();
    let mut samples = Vec::new();
    let mut failures = Vec::new();
    while let Some(done) = running.join_next().await {
        match done {
            Ok((session, measured, failure)) => {
                sessions.push(session);
                samples.extend(measured);
                failures.extend(failure);
            }
            Err(error) => failures.push(error.to_string()),
        }
    }
    Reads {
        sessions,
        latencies: Latencies::of(samples),
        failures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_percentiles_are_ranks_of_the_sorted_measures() {
        let samples: Vec<Duration> = (1..=100).rev().map(Duration::from_millis).collect();
        let latencies = Latencies::of(samples);
        assert_eq!(latencies.count, 100);
        assert_eq!(latencies.p50, Duration::from_millis(50));
        assert_eq!(latencies.p95, Duration::from_millis(95));
        assert_eq!(latencies.p99, Duration::from_millis(99));
        assert_eq!(latencies.max, Duration::from_millis(100));
        assert_eq!(Latencies::of(Vec::new()), Latencies::default());
        let one = Latencies::of(vec![Duration::from_millis(7)]);
        assert_eq!(
            (one.p50, one.p99),
            (Duration::from_millis(7), Duration::from_millis(7))
        );
    }

    #[test]
    fn the_wait_stays_below_a_ceiling_that_doubles() {
        let mut rng = SeededRng::new(7);
        let backoff = Backoff {
            first_ms: 100,
            max_ms: 1_000,
        };
        for failures in 0..40 {
            let ceiling = (100u64 << failures.min(20)).min(1_000);
            let wait = backoff.wait(failures, &mut rng);
            assert!(
                wait < Duration::from_millis(ceiling),
                "{failures}: {wait:?}"
            );
        }
    }
}
