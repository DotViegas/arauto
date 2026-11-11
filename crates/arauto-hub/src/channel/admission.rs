//! The ceiling of connections without a session (`docs/14-protocolo-do-canal.md`, section 4).
//!
//! A connection is open to anyone until its session starts, so the hub keeps at most
//! 1,024 of them, and 16 per origin. The ones waiting for approval do not count: a pending
//! device may wait for minutes. The ceiling is applied before the websocket is opened: above
//! the one of the hub the answer is 503, above the one of the origin, 429.
//!
//! An origin is an IPv4 address, or the first 64 bits of an IPv6 address: one network of an
//! IPv6 user hands out more addresses than any ceiling.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Where connections come from, as the ceiling counts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Origin(IpAddr);

impl Origin {
    /// The origin of an address.
    #[must_use]
    pub fn of(address: IpAddr) -> Self {
        match address {
            IpAddr::V4(v4) => Self(IpAddr::V4(v4)),
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => Self(IpAddr::V4(v4)),
                None => {
                    let network = u128::from(v6) & (u128::MAX << 64);
                    Self(IpAddr::V6(network.into()))
                }
            },
        }
    }
}

/// Why a connection was not let in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The hub has its ceiling of connections without a session: 503.
    HubFull,
    /// This origin has its ceiling: 429.
    OriginFull,
}

#[derive(Debug, Default)]
struct Counts {
    total: usize,
    by_origin: HashMap<Origin, usize>,
}

/// The counts of the whole hub.
#[derive(Debug)]
pub(crate) struct Admission {
    hub_max: usize,
    origin_max: usize,
    counts: Mutex<Counts>,
}

impl Admission {
    pub(crate) fn new(hub_max: usize, origin_max: usize) -> Arc<Self> {
        Arc::new(Self {
            hub_max,
            origin_max,
            counts: Mutex::new(Counts::default()),
        })
    }

    fn counts(&self) -> MutexGuard<'_, Counts> {
        // The counts are plain numbers: a panic elsewhere leaves them usable.
        self.counts.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Takes a place for a connection from `origin`, or says why there is none.
    pub(crate) fn admit(self: &Arc<Self>, origin: Origin) -> Result<Permit, Refused> {
        let mut counts = self.counts();
        if counts.total >= self.hub_max {
            return Err(Refused::HubFull);
        }
        let of_origin = counts.by_origin.get(&origin).copied().unwrap_or(0);
        if of_origin >= self.origin_max {
            return Err(Refused::OriginFull);
        }
        Self::take(&mut counts, origin);
        Ok(Permit {
            admission: Arc::clone(self),
            origin,
        })
    }

    /// Takes a place without looking at the ceiling: a connection that stops waiting goes
    /// back to the count it had left. Refusing it there would close a pending device the
    /// admin just approved.
    pub(crate) fn readmit(self: &Arc<Self>, origin: Origin) -> Permit {
        Self::take(&mut self.counts(), origin);
        Permit {
            admission: Arc::clone(self),
            origin,
        }
    }

    fn take(counts: &mut Counts, origin: Origin) {
        counts.total = counts.total.saturating_add(1);
        let of_origin = counts.by_origin.entry(origin).or_insert(0);
        *of_origin = of_origin.saturating_add(1);
    }

    fn give_back(&self, origin: Origin) {
        let mut counts = self.counts();
        counts.total = counts.total.saturating_sub(1);
        if let Some(of_origin) = counts.by_origin.get_mut(&origin) {
            *of_origin = of_origin.saturating_sub(1);
            if *of_origin == 0 {
                counts.by_origin.remove(&origin);
            }
        }
    }

    /// Connections counted right now, in the hub and for one origin.
    #[cfg(test)]
    fn counted(&self, origin: Origin) -> (usize, usize) {
        let counts = self.counts();
        (
            counts.total,
            counts.by_origin.get(&origin).copied().unwrap_or(0),
        )
    }
}

/// A place in the count. Dropped, the place is given back.
#[derive(Debug)]
pub(crate) struct Permit {
    admission: Arc<Admission>,
    origin: Origin,
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.admission.give_back(self.origin);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn v4(last: u8) -> Origin {
        Origin::of(IpAddr::V4(Ipv4Addr::new(192, 0, 2, last)))
    }

    #[test]
    fn the_ceiling_of_an_origin_is_429_and_the_one_of_the_hub_is_503() {
        let admission = Admission::new(3, 2);
        let first = admission.admit(v4(1)).unwrap();
        let _second = admission.admit(v4(1)).unwrap();
        assert_eq!(admission.admit(v4(1)).unwrap_err(), Refused::OriginFull);
        let _third = admission.admit(v4(2)).unwrap();
        assert_eq!(admission.admit(v4(3)).unwrap_err(), Refused::HubFull);
        drop(first);
        assert_eq!(admission.counted(v4(1)), (2, 1));
        assert!(admission.admit(v4(1)).is_ok());
    }

    #[test]
    fn a_place_given_back_is_free_again_and_nothing_is_left_behind() {
        let admission = Admission::new(1, 1);
        let permit = admission.admit(v4(9)).unwrap();
        assert_eq!(admission.counted(v4(9)), (1, 1));
        drop(permit);
        assert_eq!(admission.counted(v4(9)), (0, 0));
        assert!(admission.counts().by_origin.is_empty());
        let readmitted = admission.readmit(v4(9));
        // Above the ceiling, on purpose: the waiting one comes back.
        let again = admission.readmit(v4(9));
        assert_eq!(admission.counted(v4(9)), (2, 2));
        drop((readmitted, again));
        assert_eq!(admission.counted(v4(9)), (0, 0));
    }

    #[test]
    fn an_ipv6_network_is_one_origin() {
        let a: Ipv6Addr = "2001:db8:1:2:aaaa::1".parse().unwrap();
        let b: Ipv6Addr = "2001:db8:1:2:bbbb::2".parse().unwrap();
        let c: Ipv6Addr = "2001:db8:1:3::1".parse().unwrap();
        assert_eq!(Origin::of(a.into()), Origin::of(b.into()));
        assert_ne!(Origin::of(a.into()), Origin::of(c.into()));
        let mapped: Ipv6Addr = "::ffff:192.0.2.1".parse().unwrap();
        assert_eq!(Origin::of(mapped.into()), v4(1));
    }
}
