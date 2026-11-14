//! The ceilings of the panel on what a visitor who has no session can ask: looking a code up
//! and asking for a passkey challenge.
//!
//! Each of the two has its own bucket, so a flood of one never stops the other. A bucket has a
//! ceiling per client address and a larger ceiling for everyone together: one address that
//! floods is stopped alone, and the one who types a code from another address is not. The
//! map of addresses is bounded: when it is full, an address it does not know yet is counted
//! only against the ceiling of everyone together. An IPv6 address is counted by its /64: one
//! machine usually holds the whole network.
//!
//! A code has 160 bits: these ceilings do not protect a code, they keep a flood away from the
//! database and from the memory of the ceremonies. A lookup of a code is one read of a digest,
//! so its ceiling for everyone is the larger. What is not guaranteed: enough addresses together
//! still fill the ceiling of everyone, and while they do, nobody types a code
//! (`docs/05-modelo-de-ameacas.md`).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, PoisonError};

use arauto_protocol::limits::MINUTE_MS;

/// How many times a minute one address may ask.
pub(super) const PER_ADDRESS_PER_MINUTE: u32 = 30;

/// How many times a minute everyone together may look a code up.
pub(super) const CODES_GLOBAL_PER_MINUTE: u32 = 600;

/// How many times a minute everyone together may ask for a challenge: each one is kept in
/// memory until it is answered or dies.
pub(super) const CHALLENGES_GLOBAL_PER_MINUTE: u32 = 120;

/// How many addresses a bucket remembers at once.
pub(super) const ADDRESSES_MAX: usize = 4_096;

/// What is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// Looking an installation code up (`POST /sign-in`).
    Codes,
    /// Asking for or answering a passkey challenge of the entry.
    Challenges,
}

/// The minute being counted and how many were asked in it.
#[derive(Debug, Clone, Copy, Default)]
struct Window {
    start: i64,
    count: u32,
}

impl Window {
    /// Counts one more. `false` when this minute already had its share.
    fn take(&mut self, now: i64, max: u32) -> bool {
        if now.saturating_sub(self.start) >= MINUTE_MS || now < self.start {
            *self = Window {
                start: now,
                count: 0,
            };
        }
        if self.count >= max {
            return false;
        }
        self.count = self.count.saturating_add(1);
        true
    }

    fn expired(&self, now: i64) -> bool {
        now.saturating_sub(self.start) >= MINUTE_MS || now < self.start
    }
}

#[derive(Debug, Default)]
struct Bucket {
    everyone: Mutex<Window>,
    addresses: Mutex<HashMap<IpAddr, Window>>,
}

/// The key an address is counted under: an IPv4 address, or the /64 of an IPv6 one.
fn key_of(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V4(v4) => IpAddr::V4(v4),
        IpAddr::V6(v6) => {
            let mut segments = v6.segments();
            for segment in segments.iter_mut().skip(4) {
                *segment = 0;
            }
            IpAddr::V6(segments.into())
        }
    }
}

impl Bucket {
    fn allow(&self, address: Option<IpAddr>, now: i64, limits: Limits) -> bool {
        if let Some(address) = address.map(key_of) {
            let mut known = self
                .addresses
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if known.len() >= limits.addresses_max && !known.contains_key(&address) {
                // What sat past its minute is forgotten; if the map is still full, this
                // address is counted only with everyone.
                known.retain(|_, window| !window.expired(now));
            }
            if known.len() < limits.addresses_max || known.contains_key(&address) {
                let window = known.entry(address).or_default();
                // An address that is stopped does not spend what is left for the others.
                if !window.take(now, limits.per_address) {
                    return false;
                }
            }
        }
        self.everyone
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take(now, limits.everyone)
    }
}

#[derive(Debug, Clone, Copy)]
struct Limits {
    per_address: u32,
    everyone: u32,
    addresses_max: usize,
}

/// The two buckets of the panel, each with its ceilings.
#[derive(Debug)]
pub(super) struct Attempts {
    codes: Bucket,
    challenges: Bucket,
    codes_limits: Limits,
    challenges_limits: Limits,
}

impl Default for Attempts {
    fn default() -> Self {
        let limits = |everyone| Limits {
            per_address: PER_ADDRESS_PER_MINUTE,
            everyone,
            addresses_max: ADDRESSES_MAX,
        };
        Self {
            codes: Bucket::default(),
            challenges: Bucket::default(),
            codes_limits: limits(CODES_GLOBAL_PER_MINUTE),
            challenges_limits: limits(CHALLENGES_GLOBAL_PER_MINUTE),
        }
    }
}

impl Attempts {
    /// The same ceilings for the two buckets: for the tests of the counting itself.
    #[cfg(test)]
    fn with_limits(per_address: u32, everyone: u32, addresses_max: usize) -> Self {
        let limits = Limits {
            per_address,
            everyone,
            addresses_max,
        };
        Self {
            codes: Bucket::default(),
            challenges: Bucket::default(),
            codes_limits: limits,
            challenges_limits: limits,
        }
    }

    /// Counts one ask of `kind` from `address` (`None` when the hub could not tell it: only the
    /// ceiling of everyone applies). `false` when it is over a ceiling.
    pub(super) fn allow(&self, kind: Kind, address: Option<IpAddr>, now: i64) -> bool {
        match kind {
            Kind::Codes => self.codes.allow(address, now, self.codes_limits),
            Kind::Challenges => self.challenges.allow(address, now, self.challenges_limits),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: i64 = 1_800_000_000_000;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn one_address_that_floods_is_stopped_alone() {
        let attempts = Attempts::default();
        let a = Some(ip("198.51.100.1"));
        let b = Some(ip("198.51.100.2"));
        let mut allowed = 0;
        for _ in 0..100 {
            if attempts.allow(Kind::Codes, a, START) {
                allowed += 1;
            }
        }
        assert_eq!(allowed, PER_ADDRESS_PER_MINUTE);
        assert!(
            attempts.allow(Kind::Codes, b, START),
            "B is not stopped by A"
        );
        // The 70 asks of A that were refused did not spend what is left for everyone: only
        // 30 + 1 were counted, and the rest of the ceiling of everyone is still allowed.
        let mut more = 0;
        for index in 0..1_000u32 {
            let [_, _, high, low] = index.to_be_bytes();
            if attempts.allow(Kind::Codes, Some(IpAddr::from([203, 0, high, low])), START) {
                more += 1;
            }
        }
        assert_eq!(more, CODES_GLOBAL_PER_MINUTE - PER_ADDRESS_PER_MINUTE - 1);
    }

    #[test]
    fn everyone_together_has_a_larger_ceiling() {
        let attempts = Attempts::default();
        for (kind, ceiling) in [
            (Kind::Codes, CODES_GLOBAL_PER_MINUTE),
            (Kind::Challenges, CHALLENGES_GLOBAL_PER_MINUTE),
        ] {
            let mut allowed = 0;
            for index in 0..1_000u32 {
                let [_, _, high, low] = index.to_be_bytes();
                if attempts.allow(kind, Some(IpAddr::from([198, 51, high, low])), START) {
                    allowed += 1;
                }
            }
            assert_eq!(allowed, ceiling, "{kind:?}");
            assert!(ceiling > PER_ADDRESS_PER_MINUTE);
            assert!(attempts.allow(kind, None, START + MINUTE_MS));
        }
    }

    #[test]
    fn an_ipv6_network_is_one_address() {
        let attempts = Attempts::default();
        let mut allowed = 0;
        for last in 0..100u16 {
            let address = IpAddr::from([0x2001, 0xdb8, 0, 1, 0, 0, 0, last]);
            if attempts.allow(Kind::Codes, Some(address), START) {
                allowed += 1;
            }
        }
        assert_eq!(allowed, PER_ADDRESS_PER_MINUTE);
        let next_network = IpAddr::from([0x2001, 0xdb8, 0, 2, 0, 0, 0, 1]);
        assert!(attempts.allow(Kind::Codes, Some(next_network), START));
    }

    #[test]
    fn codes_and_challenges_have_their_own_buckets() {
        let attempts = Attempts::default();
        let a = Some(ip("198.51.100.1"));
        for _ in 0..PER_ADDRESS_PER_MINUTE {
            assert!(attempts.allow(Kind::Challenges, a, START));
        }
        assert!(!attempts.allow(Kind::Challenges, a, START));
        assert!(attempts.allow(Kind::Codes, a, START));
    }

    #[test]
    fn the_map_of_addresses_is_bounded_and_falls_back_to_everyone() {
        let attempts = Attempts::with_limits(2, 6, 3);
        for last in 1..=3u8 {
            assert!(attempts.allow(Kind::Codes, Some(IpAddr::from([192, 0, 2, last])), START));
        }
        // The map is full: a new address is counted with everyone only.
        let fifth = Some(IpAddr::from([192, 0, 2, 9]));
        assert!(attempts.allow(Kind::Codes, fifth, START));
        assert!(attempts.allow(Kind::Codes, fifth, START));
        assert!(
            attempts.allow(Kind::Codes, fifth, START),
            "no per-address ceiling for it"
        );
        assert!(
            !attempts.allow(Kind::Codes, fifth, START),
            "everyone is over"
        );
        // A minute later the stale entries are let go (that is what `retain` does), so the
        // address is known again and has its own ceiling: two, and then it is refused while the
        // ceiling of everyone still has room.
        let later = START + MINUTE_MS;
        assert!(attempts.allow(Kind::Codes, fifth, later));
        assert!(attempts.allow(Kind::Codes, fifth, later));
        assert!(
            !attempts.allow(Kind::Codes, fifth, later),
            "the address is counted for itself again"
        );
        assert!(attempts.allow(Kind::Codes, Some(IpAddr::from([192, 0, 2, 10])), later));
        let known = attempts.codes.addresses.lock().unwrap();
        assert!(known.len() <= 3);
        assert!(known.contains_key(&IpAddr::from([192, 0, 2, 9])));
    }

    #[test]
    fn a_clock_that_went_back_does_not_lock_the_entry() {
        let attempts = Attempts::with_limits(1, 1, 8);
        assert!(attempts.allow(Kind::Codes, None, START));
        assert!(!attempts.allow(Kind::Codes, None, START + 1));
        assert!(attempts.allow(Kind::Codes, None, START - 10 * MINUTE_MS));
    }
}
