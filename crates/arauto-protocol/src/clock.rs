//! Time, and reproducible randomness for tests.
//!
//! Time in Arauto is an `i64` of milliseconds since the Unix epoch, in UTC, and always comes from
//! a [`Clock`]. Nothing else in this crate reads the system time: the state machines receive the
//! time as an argument.
//!
//! With the `testing` feature the module also offers `ManualClock`, which only moves when the
//! test says so, and `SeededRng`, a seeded generator for reproducible random rounds.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Source of the current time.
pub trait Clock: Send + Sync {
    /// Milliseconds since the Unix epoch, in UTC.
    fn now_ms(&self) -> i64;
}

impl<C: Clock + ?Sized> Clock for &C {
    fn now_ms(&self) -> i64 {
        (**self).now_ms()
    }
}

impl<C: Clock + ?Sized> Clock for Arc<C> {
    fn now_ms(&self) -> i64 {
        (**self).now_ms()
    }
}

impl<C: Clock + ?Sized> Clock for Box<C> {
    fn now_ms(&self) -> i64 {
        (**self).now_ms()
    }
}

/// The clock of the operating system.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(after) => i64::try_from(after.as_millis()).unwrap_or(i64::MAX),
            Err(before) => i64::try_from(before.duration().as_millis()).map_or(i64::MIN, |ms| -ms),
        }
    }
}

#[cfg(any(test, feature = "testing"))]
pub use testing::{ManualClock, SeededRng};

#[cfg(any(test, feature = "testing"))]
mod testing {
    use super::Clock;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    /// A clock that only moves when the test moves it. Clones share the same time.
    #[derive(Debug, Clone)]
    pub struct ManualClock {
        now: Arc<AtomicI64>,
    }

    impl ManualClock {
        /// Creates a clock stopped at `start_ms`.
        #[must_use]
        pub fn new(start_ms: i64) -> Self {
            Self {
                now: Arc::new(AtomicI64::new(start_ms)),
            }
        }

        /// Sets the time.
        pub fn set(&self, now_ms: i64) {
            self.now.store(now_ms, Ordering::SeqCst);
        }

        /// Moves the time by `delta_ms`, which may be negative, and returns the new time.
        /// The result saturates instead of overflowing.
        pub fn advance(&self, delta_ms: i64) -> i64 {
            let moved = |now: i64| now.saturating_add(delta_ms);
            match self
                .now
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |now| Some(moved(now)))
            {
                Ok(before) | Err(before) => moved(before),
            }
        }
    }

    impl Clock for ManualClock {
        fn now_ms(&self) -> i64 {
            self.now.load(Ordering::SeqCst)
        }
    }

    /// Seeded pseudo-random generator for reproducible test rounds.
    ///
    /// **Never use it for a secret**: the whole sequence follows from the seed. Secrets come
    /// from the generator of the operating system. The algorithm is xoshiro256**, with the
    /// state filled by SplitMix64, and does not change: a seed printed by a failing test
    /// replays the same round.
    #[derive(Debug, Clone)]
    pub struct SeededRng {
        seed: u64,
        state: [u64; 4],
    }

    impl SeededRng {
        /// Creates the generator for `seed`.
        #[must_use]
        pub fn new(seed: u64) -> Self {
            let mut counter = seed;
            let mut split_mix = || {
                counter = counter.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = counter;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^ (z >> 31)
            };
            let state = [split_mix(), split_mix(), split_mix(), split_mix()];
            Self { seed, state }
        }

        /// The seed this generator was created with. Print it when a round fails.
        #[must_use]
        pub fn seed(&self) -> u64 {
            self.seed
        }

        /// Next 64 random bits.
        pub fn next_u64(&mut self) -> u64 {
            let [s0, s1, s2, s3] = self.state;
            let result = s1.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
            let shifted = s1 << 17;
            let s2 = s2 ^ s0;
            let s3 = s3 ^ s1;
            let s1 = s1 ^ s2;
            let s0 = s0 ^ s3;
            let s2 = s2 ^ shifted;
            let s3 = s3.rotate_left(45);
            self.state = [s0, s1, s2, s3];
            result
        }

        /// Uniform number in `0..bound`, without bias. Returns 0 when `bound` is 0.
        pub fn below(&mut self, bound: u64) -> u64 {
            if bound == 0 {
                return 0;
            }
            // 2^64 mod bound values at the top are refused, so every remainder is as likely.
            let refused = (u64::MAX % bound + 1) % bound;
            let last_accepted = u64::MAX - refused;
            loop {
                let value = self.next_u64();
                if value <= last_accepted {
                    return value % bound;
                }
            }
        }

        /// Uniform number in `low..=high`. Returns `low` when the range is empty.
        pub fn between(&mut self, low: i64, high: i64) -> i64 {
            if high <= low {
                return low;
            }
            let span = high.abs_diff(low);
            let offset = match span.checked_add(1) {
                Some(count) => self.below(count),
                None => self.next_u64(),
            };
            low.wrapping_add_unsigned(offset)
        }

        /// True with probability `numerator / denominator`.
        pub fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
            self.below(denominator) < numerator
        }

        /// Fills `dest` with random bytes.
        pub fn fill(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(8) {
                let bytes = self.next_u64().to_le_bytes();
                for (target, source) in chunk.iter_mut().zip(bytes) {
                    *target = source;
                }
            }
        }

        /// Shuffles `items` in place.
        pub fn shuffle<T>(&mut self, items: &mut [T]) {
            for last in (1..items.len()).rev() {
                let bound = u64::try_from(last).map_or(u64::MAX, |last| last.saturating_add(1));
                let chosen = usize::try_from(self.below(bound)).unwrap_or(0);
                items.swap(last, chosen);
            }
        }

        /// One element of `items`, or `None` when it is empty.
        pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
            let bound = u64::try_from(items.len()).unwrap_or(u64::MAX);
            let chosen = usize::try_from(self.below(bound)).unwrap_or(0);
            items.get(chosen)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_is_in_this_century() {
        let now = SystemClock.now_ms();
        // 2020-01-01 and 2100-01-01, in milliseconds.
        assert!(now > 1_577_836_800_000, "{now}");
        assert!(now < 4_102_444_800_000, "{now}");
    }

    #[test]
    fn clock_works_behind_pointers() {
        // A state machine takes `impl Clock`; whoever owns the clock lends or shares it.
        fn read(clock: impl Clock) -> i64 {
            clock.now_ms()
        }
        let manual = ManualClock::new(42);
        let shared: Arc<dyn Clock> = Arc::new(manual.clone());
        let boxed: Box<dyn Clock> = Box::new(manual.clone());
        assert_eq!(read(&manual), 42);
        assert_eq!(read(Arc::clone(&shared)), 42);
        assert_eq!(read(&shared), 42);
        assert_eq!(read(boxed), 42);
        assert_eq!(read(SystemClock).signum(), 1);
    }

    #[test]
    fn manual_clock_moves_only_by_hand() {
        let clock = ManualClock::new(1_000);
        let other = clock.clone();
        assert_eq!(clock.now_ms(), 1_000);
        assert_eq!(clock.advance(500), 1_500);
        assert_eq!(other.now_ms(), 1_500, "clones share the time");
        assert_eq!(clock.advance(-2_000), -500);
        other.set(7);
        assert_eq!(clock.now_ms(), 7);
    }

    #[test]
    fn manual_clock_saturates() {
        let clock = ManualClock::new(i64::MAX - 1);
        assert_eq!(clock.advance(10), i64::MAX);
        clock.set(i64::MIN + 1);
        assert_eq!(clock.advance(-10), i64::MIN);
    }

    /// Reference values computed outside this crate (xoshiro256** seeded with SplitMix64). If
    /// the algorithm changed, an old failing seed would stop replaying the same round.
    #[test]
    fn seeded_rng_matches_the_reference_values() {
        let mut rng = SeededRng::new(0);
        assert_eq!(rng.seed(), 0);
        let first: Vec<u64> = (0..4).map(|_| rng.next_u64()).collect();
        assert_eq!(
            first,
            [
                0x99EC_5F36_CB75_F2B4,
                0xBF6E_1F78_4956_452A,
                0x1A5F_849D_4933_E6E0,
                0x6AA5_94F1_262D_2D2C,
            ]
        );
        let mut rng = SeededRng::new(0xA5A5_5A5A_0123_4567);
        let second: Vec<u64> = (0..2).map(|_| rng.next_u64()).collect();
        assert_eq!(second, [0x2B19_B436_79FA_C1D5, 0xF550_7925_C236_3618]);
    }

    #[test]
    fn seeded_rng_repeats_for_the_same_seed() {
        let mut a = SeededRng::new(7);
        let mut b = SeededRng::new(7);
        let mut c = SeededRng::new(8);
        let from_a: Vec<u64> = (0..32).map(|_| a.next_u64()).collect();
        let from_b: Vec<u64> = (0..32).map(|_| b.next_u64()).collect();
        let from_c: Vec<u64> = (0..32).map(|_| c.next_u64()).collect();
        assert_eq!(from_a, from_b);
        assert_ne!(from_a, from_c);
    }

    #[test]
    fn below_stays_in_range_and_reaches_every_value() {
        let mut rng = SeededRng::new(1);
        let mut seen = [false; 7];
        for _ in 0..2_000 {
            let value = usize::try_from(rng.below(7)).unwrap();
            seen[value] = true;
        }
        assert_eq!(seen, [true; 7]);
        assert_eq!(rng.below(0), 0);
        assert_eq!(rng.below(1), 0);
        // A bound above half the range exercises the refusal of the biased top.
        let big = u64::MAX / 2 + 3;
        for _ in 0..200 {
            assert!(rng.below(big) < big);
        }
    }

    #[test]
    fn between_is_inclusive_and_handles_the_edges() {
        let mut rng = SeededRng::new(2);
        let mut low_seen = false;
        let mut high_seen = false;
        for _ in 0..2_000 {
            let value = rng.between(-2, 2);
            assert!((-2..=2).contains(&value));
            low_seen |= value == -2;
            high_seen |= value == 2;
        }
        assert!(low_seen && high_seen);
        assert_eq!(rng.between(5, 5), 5);
        assert_eq!(rng.between(9, 3), 9, "empty range gives the lower end");
        // The widest range must not overflow.
        let _ = rng.between(i64::MIN, i64::MAX);
        let value = rng.between(i64::MAX - 1, i64::MAX);
        assert!(value >= i64::MAX - 1);
    }

    #[test]
    fn chance_respects_the_extremes() {
        let mut rng = SeededRng::new(3);
        for _ in 0..200 {
            assert!(!rng.chance(0, 10));
            assert!(rng.chance(10, 10));
        }
        let hits = (0..4_000).filter(|_| rng.chance(1, 4)).count();
        assert!((800..1_200).contains(&hits), "{hits}");
    }

    #[test]
    fn fill_shuffle_and_pick_are_reproducible() {
        let mut a = SeededRng::new(4);
        let mut b = SeededRng::new(4);
        let mut bytes_a = [0u8; 37];
        let mut bytes_b = [0u8; 37];
        a.fill(&mut bytes_a);
        b.fill(&mut bytes_b);
        assert_eq!(bytes_a, bytes_b);
        assert_ne!(bytes_a, [0u8; 37]);

        let mut items: Vec<u32> = (0..50).collect();
        a.shuffle(&mut items);
        let mut sorted = items.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            (0..50).collect::<Vec<u32>>(),
            "a shuffle keeps every element"
        );
        assert_ne!(items, sorted, "50 elements almost never stay in order");

        let empty: [u8; 0] = [];
        assert_eq!(a.pick(&empty), None);
        assert!(items.contains(a.pick(&items).unwrap()));
    }
}
