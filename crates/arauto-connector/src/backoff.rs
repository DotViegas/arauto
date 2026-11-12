//! What follows an attempt of the channel, and how long to wait for the next one
//! (`docs/14-protocolo-do-canal.md`, sections 4 and 7; `docs/10-maquinas-de-estado.md`,
//! section 6).
//!
//! | End | Next |
//! |---|---|
//! | an established connection (waiting or in session) dropped, without `closing` | at once (C9) |
//! | AR-208 | at once (C9): if someone took the place, it shows on the way back |
//! | AR-205 | a wait drawn from 1 to 10 seconds |
//! | AR-206 | a wait drawn from 5 to 15 seconds |
//! | AR-204, and every failure before the connection was established | the growing wait: drawn between zero and a ceiling that doubles from 1 to 60 seconds |
//! | AR-301, AR-302, AR-303, AR-401, AR-402, AR-501 to AR-507 | the channel is no longer used for that device |
//!
//! After an attempt that sent `present` or `enroll` and was closed before it was established,
//! the next one with the same identity waits at least 6 seconds: the answer of the abandoned
//! attempt never meets the new one half way. One attempt at a time.
//!
//! The waits are drawn by the generator of the system; under the `testing` feature a test may
//! draw them from a seed, to repeat a round.

use std::time::Duration;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::Deadlines;

use crate::settings::{Settings, duration};

/// How an attempt ended, as the reconnection sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ended {
    /// The code of the `closing` of the hub, when it sent one.
    pub(crate) closing: Option<ErrorCode>,
    /// The connection was established: waiting for approval, or in session.
    pub(crate) established: bool,
    /// `present` was sent on it.
    pub(crate) presented: bool,
    /// The hub refused the websocket with 429 or 503: it is full or stopping, as AR-205.
    pub(crate) busy: bool,
}

/// What follows an attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Next {
    /// Another attempt, after a wait of this kind.
    Retry(Wait),
    /// The channel is no longer used for this device, with this code.
    Stop(ErrorCode),
}

/// The kinds of wait before another attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wait {
    /// At once.
    Now,
    /// The growing wait.
    Growing,
    /// From 1 to 10 seconds: the hub is stopping or full.
    HubBusy,
    /// From 5 to 15 seconds: a newer attempt of the device took this one's place.
    Replaced,
}

/// What follows an attempt that ended like `ended`.
pub(crate) fn next(ended: &Ended) -> Next {
    if ended.busy {
        return Next::Retry(Wait::HubBusy);
    }
    match ended.closing {
        None if ended.established => Next::Retry(Wait::Now),
        None => Next::Retry(Wait::Growing),
        Some(code) => match code {
            ErrorCode::Ar208 => Next::Retry(Wait::Now),
            ErrorCode::Ar205 => Next::Retry(Wait::HubBusy),
            ErrorCode::Ar206 => Next::Retry(Wait::Replaced),
            ErrorCode::Ar301
            | ErrorCode::Ar302
            | ErrorCode::Ar303
            | ErrorCode::Ar401
            | ErrorCode::Ar402
            | ErrorCode::Ar501
            | ErrorCode::Ar502
            | ErrorCode::Ar503
            | ErrorCode::Ar504
            | ErrorCode::Ar506
            | ErrorCode::Ar507 => Next::Stop(code),
            // AR-204, and any code the hub has no reason to close a channel with: again,
            // slowly.
            _ => Next::Retry(Wait::Growing),
        },
    }
}

/// Draws the waits.
#[derive(Debug)]
pub(crate) struct Jitter {
    #[cfg(feature = "testing")]
    seeded: Option<arauto_protocol::clock::SeededRng>,
}

impl Jitter {
    pub(crate) fn new(settings: &Settings) -> Self {
        #[cfg(not(feature = "testing"))]
        let _ = settings;
        Self {
            #[cfg(feature = "testing")]
            seeded: settings
                .testing
                .seed
                .map(arauto_protocol::clock::SeededRng::new),
        }
    }

    /// A number from `low` to `high`, both included. When the generator fails, `high`: the
    /// longest wait is the safe one.
    pub(crate) fn between(&mut self, low: u64, high: u64) -> u64 {
        if high <= low {
            return low;
        }
        let span = high - low;
        #[cfg(feature = "testing")]
        if let Some(seeded) = &mut self.seeded {
            return low.saturating_add(seeded.below(span.saturating_add(1)));
        }
        let Some(count) = span.checked_add(1) else {
            return getrandom::u64().unwrap_or(high);
        };
        // Values at the top that would bias the remainder are drawn again.
        let refused = (u64::MAX % count + 1) % count;
        let last_accepted = u64::MAX - refused;
        for _ in 0..64 {
            match getrandom::u64() {
                Ok(value) if value <= last_accepted => return low.saturating_add(value % count),
                Ok(_) => {}
                Err(_) => return high,
            }
        }
        high
    }
}

/// The waits of one device, in a row of attempts.
#[derive(Debug)]
pub(crate) struct Backoff {
    deadlines: Deadlines,
    jitter: Jitter,
    /// Attempts in a row that were not established.
    failures: u32,
}

fn ms(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

impl Backoff {
    pub(crate) fn new(settings: &Settings) -> Self {
        Self {
            deadlines: settings.deadlines.clone(),
            jitter: Jitter::new(settings),
            failures: 0,
        }
    }

    /// The ceiling of the growing wait after `failures` attempts in a row: the first ceiling,
    /// doubled at each one, up to the last.
    fn ceiling(&self, failures: u32) -> u64 {
        let first = ms(self.deadlines.reconnect_backoff_min_ms);
        let last = ms(self.deadlines.reconnect_backoff_max_ms);
        let doublings = failures.saturating_sub(1).min(32);
        first.saturating_mul(1u64 << doublings).min(last)
    }

    /// How long to wait before the attempt that follows `ended`, which ended with `wait`.
    pub(crate) fn wait(&mut self, ended: &Ended, wait: Wait) -> Duration {
        if ended.established {
            self.failures = 0;
        }
        let drawn = match wait {
            Wait::Now => 0,
            Wait::Growing => {
                self.failures = self.failures.saturating_add(1);
                let ceiling = self.ceiling(self.failures);
                self.jitter.between(0, ceiling)
            }
            Wait::HubBusy => self.jitter.between(
                ms(self.deadlines.hub_stopping_retry_min_ms),
                ms(self.deadlines.hub_stopping_retry_max_ms),
            ),
            Wait::Replaced => self.jitter.between(
                ms(self.deadlines.replaced_retry_min_ms),
                ms(self.deadlines.replaced_retry_max_ms),
            ),
        };
        // The answer of an abandoned attempt never meets the next one with the same identity.
        let gap = if ended.presented && !ended.established {
            ms(self.deadlines.same_identity_retry_gap_ms)
        } else {
            0
        };
        Duration::from_millis(drawn.max(gap))
    }
}

/// The interval at which a connector too old, or a hub too old, is tried again.
pub(crate) fn version_retry(settings: &Settings) -> Duration {
    duration(settings.deadlines.outdated_presentation_interval_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ended(closing: Option<ErrorCode>, established: bool) -> Ended {
        Ended {
            closing,
            established,
            presented: true,
            busy: false,
        }
    }

    #[cfg(feature = "testing")]
    fn seeded(seed: u64) -> Backoff {
        Backoff::new(&Settings::default().with_seed(seed))
    }

    /// The table of `docs/14-protocolo-do-canal.md`, section 7.
    #[test]
    fn each_end_has_its_next_step() {
        assert_eq!(next(&ended(None, true)), Next::Retry(Wait::Now));
        assert_eq!(next(&ended(None, false)), Next::Retry(Wait::Growing));
        assert_eq!(
            next(&ended(Some(ErrorCode::Ar208), true)),
            Next::Retry(Wait::Now)
        );
        assert_eq!(
            next(&ended(Some(ErrorCode::Ar204), true)),
            Next::Retry(Wait::Growing)
        );
        assert_eq!(
            next(&ended(Some(ErrorCode::Ar205), false)),
            Next::Retry(Wait::HubBusy)
        );
        assert_eq!(
            next(&ended(Some(ErrorCode::Ar206), false)),
            Next::Retry(Wait::Replaced)
        );
        let busy = Ended {
            busy: true,
            ..ended(None, false)
        };
        assert_eq!(next(&busy), Next::Retry(Wait::HubBusy));
        for code in [
            ErrorCode::Ar301,
            ErrorCode::Ar302,
            ErrorCode::Ar303,
            ErrorCode::Ar401,
            ErrorCode::Ar402,
            ErrorCode::Ar501,
            ErrorCode::Ar502,
            ErrorCode::Ar503,
            ErrorCode::Ar504,
            ErrorCode::Ar506,
            ErrorCode::Ar507,
        ] {
            assert_eq!(next(&ended(Some(code), true)), Next::Stop(code), "{code}");
        }
        for code in [ErrorCode::Ar101, ErrorCode::Ar710, ErrorCode::Ar203] {
            assert_eq!(
                next(&ended(Some(code), false)),
                Next::Retry(Wait::Growing),
                "{code}"
            );
        }
    }

    /// Full jitter: each wait is drawn between zero and a ceiling that doubles from 1 to 60
    /// seconds; an established connection that drops comes back at once and starts over.
    #[cfg(feature = "testing")]
    #[test]
    fn the_growing_wait_is_drawn_under_a_doubling_ceiling() {
        let mut backoff = seeded(7);
        let failed = Ended {
            presented: false,
            ..ended(None, false)
        };
        let mut seen = Vec::new();
        for failures in 1..=10u32 {
            let ceiling = 1_000u64.saturating_mul(1 << (failures - 1)).min(60_000);
            let wait = backoff.wait(&failed, Wait::Growing).as_millis();
            assert!(
                wait <= u128::from(ceiling),
                "{failures}: {wait} > {ceiling}"
            );
            seen.push(wait);
        }
        // Drawn, not fixed.
        seen.sort_unstable();
        seen.dedup();
        assert!(seen.len() > 5, "{seen:?}");
        // Back to the session, and dropped: at once, and the ceiling starts over.
        assert_eq!(backoff.wait(&ended(None, true), Wait::Now), Duration::ZERO);
        assert_eq!(backoff.failures, 0);
        assert!(backoff.wait(&failed, Wait::Growing) <= Duration::from_secs(1));
        assert!(backoff.wait(&failed, Wait::Growing) <= Duration::from_secs(2));
    }

    #[cfg(feature = "testing")]
    #[test]
    fn the_waits_of_ar205_and_ar206_stay_in_their_ranges() {
        let mut backoff = seeded(11);
        let established = ended(Some(ErrorCode::Ar205), true);
        for _ in 0..200 {
            let wait = backoff.wait(&established, Wait::HubBusy).as_millis();
            assert!((1_000..=10_000).contains(&wait), "{wait}");
            let wait = backoff.wait(&established, Wait::Replaced).as_millis();
            assert!((5_000..=15_000).contains(&wait), "{wait}");
        }
    }

    /// After an attempt that presented and was closed before it was established, the next one
    /// waits at least the gap, even when the draw says less.
    #[cfg(feature = "testing")]
    #[test]
    fn an_abandoned_attempt_is_followed_after_the_gap() {
        let mut backoff = seeded(3);
        for _ in 0..50 {
            let wait = backoff.wait(&ended(Some(ErrorCode::Ar204), false), Wait::Growing);
            assert!(wait >= Duration::from_secs(6), "{wait:?}");
        }
        // An established connection is not an abandoned attempt.
        assert_eq!(
            backoff.wait(&ended(Some(ErrorCode::Ar208), true), Wait::Now),
            Duration::ZERO
        );
    }

    #[test]
    fn the_system_generator_draws_within_the_range() {
        let mut jitter = Jitter::new(&Settings::default());
        for _ in 0..100 {
            let value = jitter.between(5, 9);
            assert!((5..=9).contains(&value));
        }
        assert_eq!(jitter.between(4, 4), 4);
        assert_eq!(jitter.between(9, 2), 9);
        let _ = jitter.between(0, u64::MAX);
    }
}
