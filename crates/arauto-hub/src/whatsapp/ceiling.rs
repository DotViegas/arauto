//! The send limit (`docs/08-permissoes.md`, section 7; decision 0020): at most 20 sendings in
//! any 60 seconds, per Hermes and per number. It keeps a number from being blocked by a
//! Hermes in a loop; it looks at neither the content nor the destination.
//!
//! - **What counts** is a call that passed every check of the hub and goes to the engine:
//!   [`arauto_protocol::tools::WhatsappOp::counts_toward_the_send_limit`]. A call the hub
//!   refused before the engine does not count; one the engine then failed does, because the
//!   hub cannot know whether something left.
//! - **The window slides**: the times of the last sendings are kept, and the refusal says in
//!   how many seconds the oldest one leaves it (`retry_after_s`).
//! - **At the same time**: taking a place is one step under a lock, so of many calls at once
//!   exactly as many as there are places go through.
//! - **One count per Hermes and number**, whichever session the call came from: a second
//!   connection of the same Hermes, or a reconnection, finds the same count. It lives in the
//!   memory of the hub: a restart of the hub starts it again, and a second hub of the same
//!   organization is not handled (`docs/15-mvp.md`, section 2).
//!
//! The time is the time of the hub, from its clock, which the tests control.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};

use arauto_protocol::errors::RetryAfter;
use arauto_protocol::limits::SECOND_MS;
use uuid::Uuid;

/// How many pairs are kept before the ones that are idle are dropped.
const PAIRS_BEFORE_PRUNE: usize = 4_096;

/// The count of sendings of each Hermes on each number.
#[derive(Debug)]
pub struct SendCeiling {
    per_window: usize,
    window_ms: i64,
    used: Mutex<HashMap<(Uuid, Uuid), VecDeque<i64>>>,
}

impl SendCeiling {
    /// At most `per_window` sendings in any `window_ms`.
    #[must_use]
    pub fn new(per_window: u32, window_ms: i64) -> Self {
        Self {
            per_window: usize::try_from(per_window).unwrap_or(usize::MAX),
            window_ms: window_ms.max(1),
            used: Mutex::new(HashMap::new()),
        }
    }

    /// Takes a place for one sending of `hermes` on `number` at `now`.
    ///
    /// # Errors
    /// In how many seconds a place frees, when there is none.
    pub fn take(&self, hermes: Uuid, number: Uuid, now: i64) -> Result<(), RetryAfter> {
        // Plain numbers: a panic elsewhere leaves them usable.
        let mut used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        if used.len() > PAIRS_BEFORE_PRUNE {
            let window = self.window_ms;
            used.retain(|_, times| {
                times
                    .back()
                    .is_some_and(|last| now.saturating_sub(*last) < window)
            });
        }
        let times = used.entry((hermes, number)).or_default();
        // A sending leaves the window `window_ms` after it was made. A clock that went back
        // keeps them in: the limit never opens because of it.
        while times
            .front()
            .is_some_and(|first| now.saturating_sub(*first) >= self.window_ms)
        {
            times.pop_front();
        }
        if times.len() < self.per_window {
            times.push_back(now);
            return Ok(());
        }
        let oldest = times.front().copied().unwrap_or(now);
        let frees_in_ms = oldest.saturating_add(self.window_ms).saturating_sub(now);
        let seconds = frees_in_ms.saturating_add(SECOND_MS - 1) / SECOND_MS;
        let seconds = u32::try_from(seconds.max(1)).unwrap_or(u32::MAX);
        Err(RetryAfter::new(seconds).unwrap_or(RetryAfter::MAX))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    const MINUTE: i64 = 60_000;

    #[test]
    fn twenty_in_a_minute_and_then_the_wait_of_the_oldest() {
        let ceiling = SendCeiling::new(20, MINUTE);
        let (hermes, number) = (Uuid::new_v4(), Uuid::new_v4());
        for second in 0..20 {
            assert!(
                ceiling.take(hermes, number, second * 1_000).is_ok(),
                "{second}"
            );
        }
        // The first one was at 0 s: it leaves the window at 60 s.
        let refused = ceiling.take(hermes, number, 30_500).unwrap_err();
        assert_eq!(refused.get(), 30);
        let refused = ceiling.take(hermes, number, 59_999).unwrap_err();
        assert_eq!(refused.get(), 1);
        assert!(ceiling.take(hermes, number, 60_000).is_ok());
        // Refusals do not count: at 61 s the second one left, and there is one place.
        assert!(ceiling.take(hermes, number, 61_000).is_ok());
        assert!(ceiling.take(hermes, number, 61_000).is_err());
    }

    #[test]
    fn each_hermes_and_each_number_has_its_own_count() {
        let ceiling = SendCeiling::new(2, MINUTE);
        let (first, second, number, other) = (
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        );
        assert!(ceiling.take(first, number, 0).is_ok());
        assert!(ceiling.take(first, number, 0).is_ok());
        assert!(ceiling.take(first, number, 0).is_err());
        assert!(ceiling.take(second, number, 0).is_ok());
        assert!(ceiling.take(first, other, 0).is_ok());
    }

    #[test]
    fn a_clock_that_goes_back_does_not_open_the_limit() {
        let ceiling = SendCeiling::new(1, MINUTE);
        let (hermes, number) = (Uuid::new_v4(), Uuid::new_v4());
        assert!(ceiling.take(hermes, number, 100_000).is_ok());
        let refused = ceiling.take(hermes, number, 10_000).unwrap_err();
        assert!(refused.get() >= 60, "{}", refused.get());
    }

    #[test]
    fn of_many_at_the_same_time_exactly_the_places_go_through() {
        let ceiling = Arc::new(SendCeiling::new(20, MINUTE));
        let (hermes, number) = (Uuid::new_v4(), Uuid::new_v4());
        let threads: Vec<_> = (0..64)
            .map(|_| {
                let ceiling = Arc::clone(&ceiling);
                std::thread::spawn(move || ceiling.take(hermes, number, 5_000).is_ok())
            })
            .collect();
        let passed = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .filter(|passed| *passed)
            .count();
        assert_eq!(passed, 20);
    }

    #[test]
    fn idle_pairs_are_dropped_and_their_count_with_them() {
        let ceiling = SendCeiling::new(1, MINUTE);
        let number = Uuid::new_v4();
        for _ in 0..=PAIRS_BEFORE_PRUNE {
            assert!(ceiling.take(Uuid::new_v4(), number, 0).is_ok());
        }
        let busy = Uuid::new_v4();
        assert!(ceiling.take(busy, number, MINUTE).is_ok());
        assert!(ceiling.take(Uuid::new_v4(), number, 2 * MINUTE).is_ok());
        let kept = ceiling.used.lock().unwrap().len();
        assert!(kept <= 3, "{kept}");
    }
}
